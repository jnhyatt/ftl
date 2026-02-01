mod bullets;
mod engines;
mod events;
mod oxygen;
mod reactor;
mod shields;
mod ship;
mod ship_system;
mod weapons;

use bevy::{
    app::ScheduleRunnerPlugin,
    ecs::{query::QueryFilter, system::RunSystemOnce},
    prelude::*,
    remote::{http::RemoteHttpPlugin, RemotePlugin},
    state::app::StatesPlugin,
};
use bevy_replicon::{
    prelude::*,
    server::visibility::{
        client_visibility::ClientVisibility, filters_mask::FilterBit, registry::FilterRegistry,
    },
    shared::replication::registry::ReplicationRegistry,
};
use bevy_replicon_renet::{
    netcode::{NetcodeServerTransport, ServerAuthentication, ServerConfig},
    renet::{ConnectionConfig, RenetServer},
    RenetChannelsExt, RepliconRenetPlugins,
};
use bullets::{
    beam_damage, bullet_traversal, projectile_collide_hull, projectile_shield_interact,
    projectile_test_dodge, projectile_timeout, BeamBundle, DelayedBeam, DelayedProjectile,
    ProjectileBundle, ShieldPierce, TraversalSpeed,
};
use common::{
    bullets::{FiredFrom, NeedsDodgeTest, WeaponDamage},
    intel::{InteriorIntel, SelfIntel, ShipIntel, SystemsIntel, WeaponChargeIntel},
    lobby::{PlayerReady, Ready, ReadyState},
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
    net::{Ipv4Addr, UdpSocket},
    time::{Duration, SystemTime},
};
use strum::IntoEnumIterator;

fn main() {
    App::new()
        .add_plugins((
            MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(Duration::from_millis(5))),
            #[cfg(debug_assertions)]
            (
                RemotePlugin::default(),
                RemoteHttpPlugin::default().with_port(15703),
            ),
            StatesPlugin,
            RepliconPlugins,
            RepliconRenetPlugins,
            protocol_plugin,
        ))
        .init_resource::<ReadyState>()
        .init_resource::<VisibilityScopes>()
        .add_systems(Startup, (setup, reset_gamestate))
        .add_systems(
            FixedUpdate,
            (
                handle_player_ready,
                (start_game, advance_startup_countdown).run_if(resource_exists::<ReadyState>),
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
        .add_observer(handle_connection)
        .add_observer(handle_disconnection)
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
        server_channels_config: channels.server_configs(),
        client_channels_config: channels.client_configs(),
        ..default()
    }));
    commands.insert_resource(NetcodeServerTransport::new(server_config, socket).unwrap());
}

pub fn handle_player_ready(
    mut events: MessageReader<FromClient<PlayerReady>>,
    mut ready_state: Option<ResMut<ReadyState>>,
    mut commands: Commands,
) {
    // Early out if there are no ready notifications, otherwise we'll trigger change
    // detection and send some useless network traffic every frame
    if events.is_empty() {
        return;
    }
    let Some(ReadyState::AwaitingClients) = ready_state.as_mut().map(|x| x.as_mut()) else {
        eprintln!("Discarding client ready notification, game has already started.");
        events.clear();
        return;
    };
    for &FromClient { client_id, .. } in events.read() {
        let ClientId::Client(client) = client_id else {
            eprintln!("Ignoring ready notification from server");
            continue;
        };
        commands.entity(client).insert(Ready);
    }
}

fn start_game(
    clients: Query<(&ConnectedClient, Has<Ready>)>,
    ready_states: Res<ReadyState>,
    mut commands: Commands,
) {
    let ReadyState::AwaitingClients = ready_states.as_ref() else {
        return;
    };
    if clients.iter().len() == 2 && clients.iter().all(|(_, ready)| ready) {
        commands.insert_resource(ReadyState::Starting {
            countdown: Duration::from_secs(5),
        });
    }
}

fn despawn_all<F: QueryFilter>(world: &mut World) {
    let to_despawn = world
        .query_filtered::<Entity, F>()
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
                            commands.spawn((
                                Name::new("Pending projectile"),
                                DelayedProjectile {
                                    remaining: Duration::from_millis(300 * i as u64),
                                    weapon: volley.weapon,
                                    target: volley.target,
                                    fired_from: FiredFrom {
                                        ship: e,
                                        weapon_index,
                                    },
                                },
                            ));
                        }
                    }
                    Some(weapons::Volley::Beam(volley)) => {
                        commands.spawn((
                            Name::new("Pending beam"),
                            DelayedBeam {
                                remaining: Duration::from_millis(150),
                                weapon: volley.weapon,
                                target: volley.target,
                                fired_from: FiredFrom {
                                    ship: e,
                                    weapon_index,
                                },
                            },
                        ));
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
                        world.spawn((
                            Name::new("Projectile"),
                            ProjectileBundle {
                                replicated: Replicated,
                                damage: WeaponDamage(info.weapon.common.damage),
                                target: info.target,
                                fired_from: info.fired_from,
                                traversal_speed: TraversalSpeed(info.weapon.shot_speed),
                                traversal_progress: default(),
                                needs_dodge_test: NeedsDodgeTest,
                                shield_pierce: ShieldPierce(info.weapon.shield_pierce),
                            },
                        ));
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
                        world.spawn(BeamBundle::new(info, ship_type));
                    });
                }
            }
            commands.entity(e).despawn();
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

fn handle_connection(connection: On<Add, AuthorizedClient>, mut commands: Commands) {
    let client = connection.entity;
    println!("Client connected: {:?}", client);
    commands.queue(move |world: &mut World| {
        init_player(world, client).unwrap();
    });
}

fn handle_disconnection(connection: On<Remove, ConnectedClient>, mut commands: Commands) {
    println!("Client disconnected: {:?}", connection.entity);
    commands.queue(reset_gamestate);
}

fn reset_gamestate(world: &mut World) {
    world.init_resource::<ReadyState>();
    despawn_all::<With<SelfIntel>>(world);

    let clients = world
        .query_filtered::<Entity, With<ConnectedClient>>()
        .iter(world)
        .collect::<Vec<_>>();
    for client in clients {
        init_player(world, client).unwrap();
    }
}

#[derive(Resource)]
struct VisibilityScopes {
    self_intel: FilterBit,
    weapon_charge_intel: FilterBit,
    systems_intel: FilterBit,
    interior_intel: FilterBit,
}

impl FromWorld for VisibilityScopes {
    fn from_world(world: &mut World) -> Self {
        world.resource_scope::<FilterRegistry, _>(|world, mut filter_registry| {
            world.resource_scope::<ReplicationRegistry, _>(|world, mut replication_registry| {
                let self_intel = filter_registry
                    .register_scope::<ComponentScope<SelfIntel>>(world, &mut replication_registry);
                let weapon_charge_intel = filter_registry
                    .register_scope::<ComponentScope<WeaponChargeIntel>>(
                        world,
                        &mut replication_registry,
                    );
                let systems_intel = filter_registry.register_scope::<ComponentScope<SystemsIntel>>(
                    world,
                    &mut replication_registry,
                );
                let interior_intel = filter_registry
                    .register_scope::<ComponentScope<InteriorIntel>>(
                        world,
                        &mut replication_registry,
                    );
                Self {
                    self_intel,
                    weapon_charge_intel,
                    systems_intel,
                    interior_intel,
                }
            })
        })
    }
}

fn update_intel_visibility(
    ships: Query<Entity, With<ShipState>>,
    mut clients: Query<(Entity, &mut ClientVisibility)>,
    visibility_scopes: Res<VisibilityScopes>,
) -> Result {
    for (client, mut visibility) in &mut clients {
        let _state = ships.get(client)?;
        let sensor_level = 1; // TODO
        for ship in &ships {
            if ship == client {
                // Clients can see their own `SelfIntel`, `WeaponChargeIntel` and `SystemsIntel`. When their
                // sensor level > 0, they can see their own `InteriorIntel`.
                visibility.set(ship, visibility_scopes.self_intel, true);
                visibility.set(ship, visibility_scopes.weapon_charge_intel, true);
                visibility.set(ship, visibility_scopes.systems_intel, true);
                visibility.set(ship, visibility_scopes.interior_intel, sensor_level > 0);
            } else {
                // Clients can see enemy `InteriorIntel` when sensor level > 1, `WeaponChargeIntel` when sensor
                // level > 2, and `SystemsIntel` when sensor level > 3.
                visibility.set(ship, visibility_scopes.self_intel, false);
                visibility.set(ship, visibility_scopes.interior_intel, sensor_level > 1);
                visibility.set(
                    ship,
                    visibility_scopes.weapon_charge_intel,
                    sensor_level > 2,
                );
                visibility.set(ship, visibility_scopes.systems_intel, sensor_level > 3);
            }
        }
    }
    Ok(())
}

fn init_player(world: &mut World, client: Entity) -> Result {
    let state = default_ship_state();
    world.entity_mut(client).insert((
        Replicated,
        ShipIntel {
            basic: state.basic_intel(),
            interior: client,
            weapon_charge: client,
            systems: client,
        },
        // Not replicated
        state.interior_intel(),
        state.weapon_charge_intel(),
        state.systems_intel(),
        state.self_intel(client),
        state,
    ));
    world
        .run_system_once::<_, (), _>(update_intel_visibility)
        .map_err(|_| "In `init_player`: Error running `update_intel_visibility`")?;
    Ok(())
}

fn default_ship_state() -> ShipState {
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
    ship
}
