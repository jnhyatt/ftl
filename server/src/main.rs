mod bullets;
mod engines;
mod events;
mod oxygen;
mod reactor;
mod shields;
mod ship;
mod ship_system;
mod weapons;

use bevy::{app::ScheduleRunnerPlugin, prelude::*};
use bevy_replicon::prelude::*;
use bevy_replicon_renet::{
    netcode::{NetcodeServerTransport, ServerAuthentication, ServerConfig},
    renet::{ConnectionConfig, RenetServer},
    RenetChannelsExt, RepliconRenetPlugins,
};
use bullets::{
    beam_damage, bullet_traversal, projectile_collide_hull, projectile_shield_interact,
    projectile_test_dodge, projectile_timeout, BeamBundle, BeamHits, DelayedBeam,
    DelayedProjectile, ProjectileBundle, ShieldPierce, TraversalSpeed,
};
use common::{
    bullets::{FiredFrom, NeedsDodgeTest, WeaponDamage},
    intel::{SelfIntel, ShipIntel},
    lobby::{PlayerReady, ReadyState},
    nav::{Cell, CrewNavStatus},
    protocol_plugin,
    ship::{Dead, SystemId},
    weapon::{Weapon, BURST_LASER_MK_I, HEAVY_LASER, PIKE_BEAM},
    Crew, CrewTask, PROTOCOL_ID,
};
use events::{
    adjust_power, crew_stations, move_weapon, set_autofire, set_beam_weapon_target, set_crew_goal,
    set_doors_open, set_projectile_weapon_target, weapon_power,
};
use ship::ShipState;
use ship_system::ShipSystem;
use std::{
    collections::HashMap,
    net::{Ipv4Addr, UdpSocket},
    time::{Duration, SystemTime},
};
use strum::IntoEnumIterator;

fn main() {
    App::new()
        .add_plugins((
            MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(Duration::from_millis(5))),
            RepliconPlugins.set(ServerPlugin {
                visibility_policy: VisibilityPolicy::Blacklist,
                ..default()
            }),
            RepliconRenetPlugins,
            protocol_plugin,
        ))
        .add_systems(Startup, (setup, reset_gamestate))
        .add_systems(
            FixedUpdate,
            (
                handle_connections,
                player_ready,
                (
                    handle_player_ready,
                    (start_game, advance_startup_countdown).run_if(resource_exists::<ReadyState>),
                ),
                (
                    adjust_power,
                    weapon_power,
                    set_projectile_weapon_target,
                    set_beam_weapon_target,
                    move_weapon,
                    set_crew_goal,
                    set_autofire,
                    set_doors_open,
                    crew_stations,
                ),
                (
                    bullet_traversal,
                    projectile_test_dodge,
                    projectile_shield_interact,
                    projectile_collide_hull,
                    projectile_timeout,
                    beam_damage,
                    update_dead,
                    (update_ships, (fire_beams, fire_projectiles)).chain(),
                )
                    .run_if(not(resource_exists::<ReadyState>)),
                (update_intel, update_intel_visibility).chain(),
            )
                .chain(),
        )
        .run();
}

fn setup(channels: Res<RepliconChannels>, mut commands: Commands) {
    let current_time = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap();
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 5000)).unwrap();
    let server_config = ServerConfig {
        current_time,
        max_clients: 2,
        protocol_id: PROTOCOL_ID,
        authentication: ServerAuthentication::Unsecure,
        public_addresses: vec![],
    };
    commands.insert_resource(RenetServer::new(ConnectionConfig {
        server_channels_config: channels.get_server_configs(),
        client_channels_config: channels.get_client_configs(),
        ..default()
    }));
    commands.insert_resource(NetcodeServerTransport::new(server_config, socket).unwrap());
}

pub fn player_ready(
    mut events: EventReader<FromClient<PlayerReady>>,
    mut ready_state: Option<ResMut<ReadyState>>,
) {
    // Early out if there are no ready notifications, otherwise we'll trigger change
    // detection and send some useless network traffic every frame
    if events.is_empty() {
        return;
    }
    let Some(ReadyState::AwaitingClients { ready_clients }) =
        ready_state.as_mut().map(|x| x.as_mut())
    else {
        eprintln!("Discarding client ready notification, game has already started.");
        return;
    };
    for &FromClient { client_id, .. } in events.read() {
        ready_clients.insert(client_id);
    }
}

#[derive(Resource, Deref, DerefMut, Debug, Default, Clone)]
pub struct ClientShips(HashMap<ClientId, Entity>);

fn handle_player_ready(
    mut events: EventReader<FromClient<PlayerReady>>,
    mut ready_state: Option<ResMut<ReadyState>>,
) {
    let Some(ReadyState::AwaitingClients { ready_clients }) =
        ready_state.as_mut().map(|x| x.as_mut())
    else {
        events.clear();
        return;
    };
    for &FromClient { client_id, .. } in events.read() {
        ready_clients.insert(client_id);
    }
}

fn start_game(
    clients: Res<ConnectedClients>,
    ready_states: Res<ReadyState>,
    mut commands: Commands,
) {
    let ReadyState::AwaitingClients { ready_clients } = ready_states.as_ref() else {
        return;
    };
    if clients.len() == 2 && clients.iter().all(|x| ready_clients.contains(&x.id())) {
        commands.insert_resource(ReadyState::Starting {
            countdown: Duration::from_secs(5),
        });
    }
}

fn despawn_all<C: Component>(world: &mut World) {
    let to_despawn = world
        .query_filtered::<Entity, With<C>>()
        .iter(world)
        .collect::<Vec<_>>();
    for e in to_despawn {
        world.entity_mut(e).despawn();
    }
}

pub fn update_ships(
    mut ships: Query<(Entity, &mut ShipState), Without<Dead>>,
    mut commands: Commands,
) {
    for (e, mut ship) in &mut ships {
        if let Some(shields) = &mut ship.systems.shields {
            shields.charge_shield();
        }
        if let Some(volleys) = ship.update_weapons() {
            for (weapon_index, volley) in volleys.enumerate() {
                match volley {
                    Some(weapons::Volley::Projectile(volley)) => {
                        for i in 0..volley.weapon.volley_size {
                            commands.spawn(DelayedProjectile {
                                remaining: Duration::from_millis(300 * i as u64),
                                weapon: volley.weapon,
                                target: volley.target,
                                fired_from: FiredFrom {
                                    ship: e,
                                    weapon_index,
                                },
                            });
                        }
                    }
                    Some(weapons::Volley::Beam(volley)) => {
                        commands.spawn(DelayedBeam {
                            remaining: Duration::from_millis(150),
                            weapon: volley.weapon,
                            target: volley.target,
                            fired_from: FiredFrom {
                                ship: e,
                                weapon_index,
                            },
                        });
                    }
                    None => {}
                }
            }
        }
        ship.update_crew();
        ship.update_repair_status();
        ship.update_oxygen();
    }
}

fn fire_projectiles(
    ships: Query<&ShipState>,
    mut pending: Query<(Entity, &mut DelayedProjectile)>,
    mut commands: Commands,
    time: Res<Time>,
) {
    for (e, mut projectile) in &mut pending {
        if let Some(new_remaining) = projectile.remaining.checked_sub(time.delta()) {
            projectile.remaining = new_remaining;
        } else {
            let ship = ships.get(projectile.fired_from.ship).unwrap();
            if let Some(weapons) = &ship.systems.weapons {
                if weapons.weapons()[projectile.fired_from.weapon_index].is_powered() {
                    commands.queue(move |world: &mut World| {
                        let info = world.entity_mut(e).take::<DelayedProjectile>().unwrap();
                        world.spawn(ProjectileBundle {
                            replicated: Replicated,
                            damage: WeaponDamage(info.weapon.common.damage),
                            target: info.target,
                            fired_from: info.fired_from,
                            traversal_speed: TraversalSpeed(info.weapon.shot_speed),
                            traversal_progress: default(),
                            needs_dodge_test: NeedsDodgeTest,
                            shield_pierce: ShieldPierce(info.weapon.shield_pierce),
                        });
                    });
                }
            }
            commands.entity(e).despawn();
        }
    }
}

fn fire_beams(
    ships: Query<&ShipState>,
    mut pending: Query<(Entity, &mut DelayedBeam)>,
    mut commands: Commands,
    time: Res<Time>,
) {
    for (e, mut beam) in &mut pending {
        if let Some(new_remaining) = beam.remaining.checked_sub(time.delta()) {
            beam.remaining = new_remaining;
        } else {
            let ship = ships.get(beam.fired_from.ship).unwrap();
            let ship_type = ship.ship_type;
            if let Some(weapons) = &ship.systems.weapons {
                // TODO When the player rearranges weapons, we'll want to make sure to adjust the
                // `weapon_index` for all entities storing it -- delayed and in-world weapon shots,
                // maybe more?
                if weapons.weapons()[beam.fired_from.weapon_index].is_powered() {
                    commands.queue(move |world: &mut World| {
                        let info = world.entity_mut(e).take::<DelayedBeam>().unwrap();
                        world.spawn(BeamBundle {
                            replicated: Replicated,
                            damage: WeaponDamage(info.weapon.common.damage),
                            target: info.target,
                            hits: BeamHits::compute(ship_type, info.weapon.length, &info.target),
                            fired_from: info.fired_from,
                            traversal_speed: TraversalSpeed(info.weapon.speed),
                            traversal_progress: default(),
                        });
                    });
                }
            }
            commands.entity(e).despawn();
        }
    }
}

fn update_intel_visibility(
    mut clients: ResMut<ReplicatedClients>,
    client_ships: Res<ClientShips>,
    self_intel: Query<(Entity, &SelfIntel)>,
    ships: Query<(Entity, &ShipIntel)>,
) {
    // For each client, make sure they only see entities based on their ship's sensors level
    for client in clients.iter_mut() {
        let client_id = client.id();
        let client_visibility = client.visibility_mut();
        let &own_ship = client_ships.get(&client_id).unwrap();

        // Hide self intel for all but owning player
        for (self_intel, SelfIntel { ship, .. }) in &self_intel {
            client_visibility.set_visibility(self_intel, own_ship == *ship);
        }

        for (ship, intel) in &ships {
            // Hardcoded for now to allow clients to see own interior
            let sensor_level = 1; // 0-4, with 4 being level 3 + manned

            if ship == own_ship {
                // Clients always get their own crew vision and operational status
                client_visibility.set_visibility(intel.crew_vision, true);
                client_visibility.set_visibility(intel.weapon_charge, true);
                client_visibility.set_visibility(intel.systems, true);
                client_visibility.set_visibility(intel.interior, sensor_level > 0);
            } else {
                client_visibility.set_visibility(intel.interior, sensor_level > 1);
                client_visibility.set_visibility(intel.weapon_charge, sensor_level > 2);
                client_visibility.set_visibility(intel.systems, sensor_level > 3);
            }
        }
    }
}

fn update_intel(
    mut ships: Query<(&ShipState, &mut ShipIntel)>,
    mut self_intel: Query<&mut SelfIntel>,
    mut commands: Commands,
) {
    for mut self_intel in &mut self_intel {
        let (ship, mut intel) = ships.get_mut(self_intel.ship).unwrap();
        *self_intel = ship.self_intel(self_intel.ship);
        intel.basic = ship.basic_intel();
        commands
            .entity(intel.crew_vision)
            .insert(ship.crew_vision_intel());
        commands
            .entity(intel.interior)
            .insert(ship.interior_intel());
        commands
            .entity(intel.weapon_charge)
            .insert(ship.weapon_charge_intel());
        commands.entity(intel.systems).insert(ship.systems_intel());
    }
}

fn update_dead(ships: Query<(Entity, &ShipState)>, mut commands: Commands) {
    for (e, ship) in &ships {
        if ship.damage == ship.max_hull {
            commands.entity(e).insert(Dead);
        }
    }
}

fn advance_startup_countdown(
    ready_state: Res<ReadyState>,
    time: Res<Time>,
    mut commands: Commands,
) {
    if let ReadyState::Starting { countdown } = ready_state.as_ref() {
        if let Some(new_countdown) = countdown.checked_sub(time.delta()) {
            commands.insert_resource(ReadyState::Starting {
                countdown: new_countdown,
            });
        } else {
            commands.remove_resource::<ReadyState>();
        }
    }
}

fn handle_connections(mut server_events: EventReader<ServerEvent>, mut commands: Commands) {
    for event in server_events.read() {
        match event {
            ServerEvent::ClientConnected { client_id } => {
                println!("New client {client_id:?} connected.");
                let client_id = *client_id;
                commands.queue(move |world: &mut World| {
                    spawn_player(world, client_id);
                });
            }
            ServerEvent::ClientDisconnected { client_id, reason } => {
                println!("Client {client_id:?} disconnected: {reason}");
                commands.queue(reset_gamestate);
            }
        }
    }
}

fn reset_gamestate(world: &mut World) {
    world.init_resource::<ReadyState>();
    world.init_resource::<ClientShips>();
    despawn_all::<ShipState>(world);
    despawn_all::<Replicated>(world);

    let clients = world
        .resource::<ConnectedClients>()
        .iter()
        .copied()
        .collect::<Vec<_>>();
    for client in clients {
        spawn_player(world, client.id());
    }
}

fn spawn_player(world: &mut World, client_id: ClientId) {
    let mut ship = ShipState::new();
    for _ in 0..8 {
        ship.reactor.upgrade();
    }

    for system in SystemId::iter() {
        ship.install_system(system);
    }

    // TODO Add a dedicated API to bring on crew
    ship.crew.push(Crew {
        race: 0,
        name: "Fish".into(),
        nav_status: CrewNavStatus::At(Cell(2)),
        health: 100.0,
        task: CrewTask::Idle,
        station: None,
    });
    ship.crew.push(Crew {
        race: 0,
        name: "Virus".into(),
        nav_status: CrewNavStatus::At(Cell(6)),
        health: 100.0,
        task: CrewTask::Idle,
        station: None,
    });
    ship.crew.push(Crew {
        race: 0,
        name: "Stick".into(),
        nav_status: CrewNavStatus::At(Cell(10)),
        health: 100.0,
        task: CrewTask::Idle,
        station: None,
    });

    let shields = ship.systems.shields.as_mut().unwrap();
    for _ in 0..3 {
        shields.upgrade();
    }
    let engines = ship.systems.engines.as_mut().unwrap();
    for _ in 0..3 {
        engines.upgrade();
    }
    let weapons = ship.systems.weapons.as_mut().unwrap();
    for _ in 0..3 {
        weapons.upgrade();
    }
    weapons.install_weapon(0, Weapon::new(HEAVY_LASER));
    weapons.install_weapon(1, Weapon::new(BURST_LASER_MK_I));
    weapons.install_weapon(2, Weapon::new(PIKE_BEAM));

    let crew_vision = world.spawn((Replicated, ship.crew_vision_intel())).id();
    let interior = world.spawn((Replicated, ship.interior_intel())).id();
    let weapon_charge = world.spawn((Replicated, ship.weapon_charge_intel())).id();
    let systems = world.spawn((Replicated, ship.systems_intel())).id();
    let ship_e = world
        .spawn((
            Replicated,
            ShipIntel {
                basic: ship.basic_intel(),
                crew_vision,
                interior,
                weapon_charge,
                systems,
            },
        ))
        .id();
    world.spawn((Replicated, ship.self_intel(ship_e)));
    world.entity_mut(ship_e).insert(ship);
    let ship = ship_e;
    world.resource_mut::<ClientShips>().insert(client_id, ship);
}
