mod engines;
mod events;
mod projectiles;
mod reactor;
mod shields;
mod ship;
mod ship_system;
mod weapons;

use bevy::{app::ScheduleRunnerPlugin, prelude::*};
use bevy_replicon::prelude::*;
use bevy_replicon_renet::{
    renet::{
        transport::{NetcodeServerTransport, ServerAuthentication, ServerConfig},
        ConnectionConfig, RenetServer,
    },
    RenetChannelsExt, RepliconRenetServerPlugin,
};
use common::{
    intel::{SelfIntel, ShipIntel},
    lobby::{PlayerReady, ReadyState},
    nav::{Cell, CrewNavStatus},
    projectiles::{FiredFrom, NeedsDodgeTest, WeaponDamage},
    protocol_plugin,
    ship::Dead,
    weapon::Weapon,
    Crew, PROTOCOL_ID,
};
use events::{
    adjust_power, move_weapon, set_autofire, set_crew_goal, set_projectile_weapon_target,
    weapon_power,
};
use projectiles::{
    projectile_collide_hull, projectile_shield_interact, projectile_test_dodge, projectile_timeout,
    projectile_traversal, Delayed, ProjectileBundle, ShieldPierce, TraversalSpeed,
};
use ship::ShipState;
use ship_system::ShipSystem;
use std::{
    collections::HashMap,
    net::{Ipv6Addr, SocketAddr, UdpSocket},
    time::{Duration, SystemTime},
};

fn main() {
    App::new()
        .add_plugins((
            MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(Duration::from_millis(5))),
            RepliconPlugins
                .build()
                .disable::<ClientPlugin>()
                .set(ServerPlugin {
                    visibility_policy: VisibilityPolicy::Blacklist,
                    ..default()
                }),
            RepliconRenetServerPlugin,
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
                    move_weapon,
                    set_crew_goal,
                    set_autofire,
                ),
                (
                    projectile_traversal,
                    projectile_test_dodge,
                    projectile_shield_interact,
                    projectile_collide_hull,
                    projectile_timeout,
                    update_dead,
                    (update_ships, fire_projectiles).chain(),
                )
                    .run_if(not(resource_exists::<ReadyState>)),
                (update_intel, update_intel_visibility).chain(),
            )
                .chain(),
        )
        .run();
}

fn setup(channels: Res<RepliconChannels>, mut commands: Commands) {
    let server_channels_config = channels.get_server_configs();
    let client_channels_config = channels.get_client_configs();
    commands.insert_resource(RenetServer::new(ConnectionConfig {
        server_channels_config,
        client_channels_config,
        ..default()
    }));

    let current_time = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap();
    let public_addr = SocketAddr::new(Ipv6Addr::UNSPECIFIED.into(), 5000);
    let socket = UdpSocket::bind(public_addr).unwrap();
    let server_config = ServerConfig {
        current_time,
        max_clients: 2,
        protocol_id: PROTOCOL_ID,
        authentication: ServerAuthentication::Unsecure,
        public_addresses: vec![public_addr],
    };
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
        if let Some((shields, _)) = &mut ship.systems.shields {
            shields.charge_shield();
        }
        if let Some(projectiles) = ship.update_weapons() {
            for (weapon_index, projectile) in projectiles.enumerate() {
                for i in 0..projectile.count {
                    commands.spawn(Delayed {
                        remaining: Duration::from_millis(300 * i as u64),
                        weapon: projectile.weapon,
                        target: projectile.target,
                        fired_from: FiredFrom {
                            ship: e,
                            weapon_index,
                        },
                    });
                }
            }
        }
        ship.update_crew();
        ship.update_repair_status();
    }
}

fn fire_projectiles(
    ships: Query<&ShipState>,
    mut pending: Query<(Entity, &mut Delayed)>,
    mut commands: Commands,
    time: Res<Time>,
) {
    for (e, mut projectile) in &mut pending {
        if let Some(new_remaining) = projectile.remaining.checked_sub(time.delta()) {
            projectile.remaining = new_remaining;
        } else {
            let ship = ships.get(projectile.fired_from.ship).unwrap();
            if let Some((weapons, _)) = &ship.systems.weapons {
                if weapons.weapons()[projectile.fired_from.weapon_index].is_powered() {
                    commands.add(move |world: &mut World| {
                        let info = world.entity_mut(e).take::<Delayed>().unwrap();
                        world.spawn(ProjectileBundle {
                            replicated: Replicated,
                            damage: WeaponDamage(info.weapon.damage),
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

fn update_intel_visibility(
    mut clients: ResMut<ConnectedClients>,
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
                commands.add(move |world: &mut World| {
                    spawn_player(world, client_id);
                });
            }
            ServerEvent::ClientDisconnected { client_id, reason } => {
                println!("Client {client_id:?} disconnected: {reason}");
                commands.add(reset_gamestate);
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
        .iter_client_ids()
        .collect::<Vec<_>>();
    for client in clients {
        spawn_player(world, client);
    }
}

fn spawn_player(world: &mut World, client_id: ClientId) {
    let mut ship = ShipState::new();
    for _ in 0..8 {
        ship.reactor.upgrade();
    }
    // TODO Move systems to specific rooms
    ship.install_shields(2);
    ship.install_engines(1);
    ship.install_weapons(3);

    // TODO Add a dedicated API to bring on crew
    ship.crew.push(Crew {
        name: "Fish".into(),
        nav_status: CrewNavStatus::At(Cell(0)),
        health: 100.0,
        max_health: 100.0,
    });
    ship.crew.push(Crew {
        name: "Virus".into(),
        nav_status: CrewNavStatus::At(Cell(4)),
        health: 100.0,
        max_health: 100.0,
    });
    ship.crew.push(Crew {
        name: "Stick".into(),
        nav_status: CrewNavStatus::At(Cell(6)),
        health: 100.0,
        max_health: 100.0,
    });

    let (shields, _) = ship.systems.shields.as_mut().unwrap();
    for _ in 0..3 {
        shields.upgrade();
    }
    let (engines, _) = ship.systems.engines.as_mut().unwrap();
    for _ in 0..3 {
        engines.upgrade();
    }
    let (weapons, _) = ship.systems.weapons.as_mut().unwrap();
    for _ in 0..3 {
        weapons.upgrade();
    }
    weapons.add_weapon(0, Weapon(2));
    weapons.add_weapon(1, Weapon(0));

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
