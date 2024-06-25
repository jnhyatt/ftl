mod events;
mod projectiles;

use std::{
    collections::HashMap,
    net::{Ipv6Addr, SocketAddr, UdpSocket},
    time::{Duration, SystemTime},
};

use bevy::prelude::*;
use bevy_replicon::prelude::*;
use bevy_replicon_renet::{
    renet::{
        transport::{NetcodeServerTransport, ServerAuthentication, ServerConfig},
        ConnectionConfig, RenetServer,
    },
    RenetChannelsExt, RepliconRenetServerPlugin,
};
use common::{projectiles::*, *};
use events::*;
use projectiles::*;

fn main() {
    App::new()
        .add_plugins((
            MinimalPlugins,
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
                    start_game.run_if(resource_exists::<ReadyState>),
                    advance_startup_countdown.run_if(resource_exists::<ReadyState>),
                ),
                (
                    adjust_power,
                    weapon_power,
                    set_projectile_weapon_target,
                    set_autofire,
                ),
                (
                    projectile_traversal,
                    projectile_test_dodge,
                    projectile_shield_interact,
                    projectile_collide_hull,
                    projectile_timeout,
                    update_dead,
                    Ship::update_ships,
                )
                    .run_if(not(resource_exists::<ReadyState>)),
                (update_intel, update_intel_visibility),
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

fn update_intel_visibility(
    mut clients: ResMut<ConnectedClients>,
    client_ships: Res<ClientShips>,
    intel_packages: Query<&IntelPackage>,
    ships: Query<(Entity, &ShipIntel)>,
) {
    // for each client, make sure they only see entities based on their ship's
    // sensors level
    for client in clients.iter_mut() {
        let client_id = client.id();
        let client_visibility = client.visibility_mut();
        for (ship, intel) in &ships {
            let intel_package = intel_packages.get(intel.0).unwrap();
            client_visibility.set_visibility(ship, false);
            client_visibility.set_visibility(intel_package.basic, true);
        }
        let (own_ship, _) = ships.get(*client_ships.get(&client_id).unwrap()).unwrap();
        client_visibility.set_visibility(own_ship, true);
    }
}

fn update_intel(
    ships: Query<(&Ship, &ShipIntel)>,
    intel_packages: Query<&IntelPackage>,
    mut commands: Commands,
) {
    for (ship, intel) in &ships {
        let intel = intel_packages.get(intel.0).unwrap();
        commands.entity(intel.basic).insert(BasicIntel::new(ship));
    }
}

fn update_dead(ships: Query<(Entity, &Ship, &ShipIntel)>, mut commands: Commands) {
    for (e, ship, intel) in &ships {
        if ship.damage == ship.max_hull {
            commands.entity(e).insert(Dead);
            commands.entity(intel.0).insert(Dead);
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
    despawn_all::<Ship>(world);
    despawn_all::<IntelPackage>(world);
    despawn_all::<BasicIntel>(world);
    despawn_all::<ProjectileTarget>(world);

    let clients = world
        .resource::<ConnectedClients>()
        .iter_client_ids()
        .collect::<Vec<_>>();
    for client in clients {
        spawn_player(world, client);
    }
}

fn spawn_player(world: &mut World, client_id: ClientId) {
    let mut ship = Ship::new();
    for _ in 0..24 {
        ship.reactor.upgrade();
    }
    ship.install_shields(0);
    ship.install_engines(1);
    ship.install_weapons(2);
    let shields = ship.systems.shields_mut().unwrap();
    for _ in 0..7 {
        shields.upgrade();
    }
    let engines = ship.systems.engines_mut().unwrap();
    for _ in 0..7 {
        engines.upgrade();
    }
    let weapons = ship.systems.weapons_mut().unwrap();
    for _ in 0..7 {
        weapons.upgrade();
    }
    weapons.add_weapon(0, Weapon(0));
    weapons.add_weapon(1, Weapon(0));
    weapons.add_weapon(2, Weapon(1));
    weapons.add_weapon(3, Weapon(1));
    weapons.add_missiles(15);

    let basic_intel = world.spawn(Replicated).id();
    let intel_package = world
        .spawn((
            Replicated,
            Name::new(format!("Potato Bug")),
            IntelPackage { basic: basic_intel },
        ))
        .id();
    let ship = world
        .spawn((
            Replicated,
            ship,
            ShipIntel(intel_package),
            Name::new(format!("Potato Bug")),
        ))
        .id();
    world.resource_mut::<ClientShips>().insert(client_id, ship);
}
