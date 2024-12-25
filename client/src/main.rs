mod egui_panels;
mod graphics;
mod interaction;
mod select;

use crate::{
    egui_panels::{
        crew_panel, enemy_panels, power_panel, ready_panel, shields_panel, status_panel,
        weapons_panel,
    },
    select::{selection_plugin, SelectEvent, SelectionEnabled},
};
use bevy::{math::vec2, prelude::*};
use bevy_egui::EguiPlugin;
use bevy_replicon::prelude::*;
use bevy_replicon_renet::{
    netcode::{ClientAuthentication, NetcodeClientTransport},
    renet::{ConnectionConfig, RenetClient},
    RenetChannelsExt as _, RepliconRenetPlugins,
};
use common::{
    events::{AdjustPower, CrewStations, PowerDir, SetAutofire, SetDoorsOpen, WeaponPower},
    intel::{SelfIntel, ShipIntel},
    lobby::ReadyState,
    protocol_plugin,
    ship::SystemId,
    util::{enable, init_resource, remove_resource},
    PROTOCOL_ID,
};
use graphics::{
    add_ship_graphic, draw_beams, draw_targets, set_bullet_incidence, spawn_projectile_graphics,
    sync_crew_count, sync_crew_positions, update_bullet_graphic, update_doors, update_no_intel,
    update_oxygen, update_vacuum,
};
use interaction::{left_click_background, start_targeting, PickRoot, TargetingWeapon};
use leafwing_input_manager::{
    action_state::ActionState,
    input_map::InputMap,
    plugin::InputManagerPlugin,
    prelude::{ButtonlikeChord, ModifierKey},
    Actionlike, InputControlKind, InputManagerBundle,
};
use std::{
    net::{Ipv4Addr, SocketAddr, UdpSocket},
    time::SystemTime,
};

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        resolution: bevy::window::WindowResolution::new(1280.0, 720.0),
                        title: "PVP: Paster Vhan Pight".into(),
                        resizable: false,
                        enabled_buttons: bevy::window::EnabledButtons {
                            maximize: false,
                            ..default()
                        },
                        ..default()
                    }),
                    ..default()
                })
                .set(ImagePlugin::default_nearest()),
            EguiPlugin,
            DefaultPickingPlugins,
            InputManagerPlugin::<Controls>::default(),
            RepliconPlugins,
            RepliconRenetPlugins,
            protocol_plugin,
            selection_plugin,
        ))
        .add_systems(Startup, connect_to_server)
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                power_panel,
                status_panel,
                weapons_panel,
                shields_panel,
                enemy_panels,
                ready_panel.run_if(resource_exists::<ReadyState>),
                add_ship_controls,
                add_ship_graphic,
                crew_panel,
            ),
        )
        .add_systems(Update, (sync_crew_count, sync_crew_positions).chain())
        .add_systems(
            Update,
            (
                set_bullet_incidence,
                spawn_projectile_graphics,
                update_bullet_graphic,
                draw_beams,
                update_doors,
                update_oxygen,
                update_vacuum,
                update_no_intel,
            ),
        )
        .add_systems(Update, (controls, draw_targets))
        .add_systems(
            Update,
            (
                init_resource::<SelectionEnabled>.run_if(resource_removed::<TargetingWeapon>),
                remove_resource::<SelectionEnabled>.run_if(resource_added::<TargetingWeapon>),
                (|pick_root: Single<Entity, With<PickRoot>>, mut commands: Commands| {
                    commands.entity(*pick_root).queue(enable::<Observer>);
                })
                .run_if(resource_removed::<TargetingWeapon>),
            ),
        )
        .run();
}

fn connect_to_server(channels: Res<RepliconChannels>, mut commands: Commands) {
    let server_channels_config = channels.get_server_configs();
    let client_channels_config = channels.get_client_configs();
    commands.insert_resource(RenetClient::new(ConnectionConfig {
        server_channels_config,
        client_channels_config,
        ..default()
    }));

    let current_time = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap();
    let client_id = current_time.as_millis() as u64;
    let server_addr = SocketAddr::new(
        // Ipv6Addr::new(0x2601, 0x680, 0xcd00, 0xbb3, 0x22a0, 0xcc7f, 0x4f4d, 0xa1a7).into(),
        Ipv4Addr::new(192, 168, 0, 28).into(),
        // Ipv6Addr::new(0x2a01, 0x4ff, 0x1f0, 0x9230, 0x0, 0x0, 0x0, 0x1).into(),
        // Ipv6Addr::LOCALHOST.into(),
        5000,
    );
    let socket = UdpSocket::bind(SocketAddr::from((Ipv4Addr::UNSPECIFIED, 0))).unwrap();
    let authentication = ClientAuthentication::Unsecure {
        client_id,
        protocol_id: PROTOCOL_ID,
        server_addr,
        user_data: None,
    };
    let transport = NetcodeClientTransport::new(current_time, authentication, socket).unwrap();
    commands.insert_resource(transport);
}

fn setup(mut commands: Commands, assets: Res<AssetServer>) {
    // Lots of sprites have x/y values that have 0 fractional part, and that can make them a little
    // temperamental in terms of which pixels they decide to occupy. If we shift the camera just a
    // quarter pixel up and right, this resolves all issues with these sprites by putting their
    // texels solidly on a pixel, rather than right on the border.
    commands.spawn((Camera2d, Msaa::Off, Transform::from_xyz(0.25, 0.25, 0.0)));
    commands.spawn(Sprite {
        image: assets.load("background-1.png"),
        ..default()
    });
    commands
        .spawn((
            PickingBehavior {
                should_block_lower: false,
                is_hoverable: true,
            },
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
        ))
        .observe(|event: Trigger<Pointer<Down>>, mut commands: Commands| {
            if event.button == PointerButton::Secondary {
                commands.remove_resource::<TargetingWeapon>();
            }
        });
    commands
        .spawn((
            PickingBehavior {
                should_block_lower: false,
                is_hoverable: true,
            },
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
        ))
        // .observe(left_click_background)
        .observe(
            |event: Trigger<Pointer<Up>>, mut select_events: EventWriter<SelectEvent>| {
                if event.button == PointerButton::Primary {
                    select_events.send(SelectEvent::Complete);
                }
                select_events.send(SelectEvent::GrowTo(
                    event.pointer_location.position * vec2(1.0, -1.0) + vec2(-640.0, 360.0),
                ));
            },
        )
        .observe(
            |event: Trigger<Pointer<Drag>>, mut select_events: EventWriter<SelectEvent>| {
                if event.button == PointerButton::Primary {
                    select_events.send(SelectEvent::GrowTo(
                        event.pointer_location.position * vec2(1.0, -1.0) + vec2(-640.0, 360.0),
                    ));
                }
            },
        );
    commands.spawn((PickRoot, Observer::new(left_click_background)));
}

fn add_ship_controls(
    self_intel: Single<&SelfIntel>,
    ships: Query<Entity, Without<Sprite>>,
    mut commands: Commands,
) {
    let my_ship = self_intel.ship;
    for ship in &ships {
        let input_map = if ship == my_ship {
            use KeyCode::*;
            use SystemId::*;
            let shift = |key| ButtonlikeChord::modified(ModifierKey::Shift, key);
            InputMap::default()
                .with(Controls::Autofire, KeyV)
                .with(Controls::AllDoors { open: true }, KeyZ)
                .with(Controls::AllDoors { open: false }, KeyX)
                .with(Controls::SaveStations, Slash)
                .with(Controls::ReturnToStations, Enter)
                .with(Controls::power_system(Shields), KeyA)
                .with(Controls::power_system(Engines), KeyS)
                .with(Controls::power_system(Weapons), KeyW)
                .with(Controls::power_system(Oxygen), KeyF)
                .with(Controls::power_weapon(0), Digit1)
                .with(Controls::power_weapon(1), Digit2)
                .with(Controls::power_weapon(2), Digit3)
                .with(Controls::power_weapon(3), Digit4)
                .with(Controls::depower_system(Shields), shift(KeyA))
                .with(Controls::depower_system(Shields), shift(KeyA))
                .with(Controls::depower_system(Engines), shift(KeyS))
                .with(Controls::depower_system(Weapons), shift(KeyW))
                .with(Controls::depower_system(Oxygen), shift(KeyF))
                .with(Controls::depower_weapon(0), shift(Digit1))
                .with(Controls::depower_weapon(1), shift(Digit2))
                .with(Controls::depower_weapon(2), shift(Digit3))
                .with(Controls::depower_weapon(3), shift(Digit4))
        } else {
            default()
        };
        commands
            .entity(ship)
            .insert(InputManagerBundle::with_map(input_map));
    }
}

#[derive(Reflect, Debug, Clone, Hash, PartialEq, Eq)]
enum Controls {
    SystemPower { dir: PowerDir, system: SystemId },
    WeaponPower { dir: PowerDir, weapon_index: usize },
    Autofire,
    AllDoors { open: bool },
    SaveStations,
    ReturnToStations,
}

impl Actionlike for Controls {
    fn input_control_kind(&self) -> InputControlKind {
        InputControlKind::Button
    }
}

impl Controls {
    fn power_system(system: SystemId) -> Self {
        let dir = PowerDir::Request;
        Self::SystemPower { dir, system }
    }

    fn depower_system(system: SystemId) -> Self {
        let dir = PowerDir::Remove;
        Self::SystemPower { dir, system }
    }

    fn power_weapon(weapon_index: usize) -> Self {
        let dir = PowerDir::Request;
        Self::WeaponPower { dir, weapon_index }
    }

    fn depower_weapon(weapon_index: usize) -> Self {
        let dir = PowerDir::Remove;
        Self::WeaponPower { dir, weapon_index }
    }
}

fn controls(
    self_intel: Query<&SelfIntel>,
    ships: Query<(&ShipIntel, &ActionState<Controls>)>,
    mut power: EventWriter<AdjustPower>,
    mut weapon_power: EventWriter<WeaponPower>,
    mut set_autofire: EventWriter<SetAutofire>,
    mut set_doors_open: EventWriter<SetDoorsOpen>,
    mut crew_stations: EventWriter<CrewStations>,
    mut commands: Commands,
) {
    let Ok(self_intel) = self_intel.get_single() else {
        return;
    };
    let Ok((ship, actions)) = ships.get(self_intel.ship) else {
        return;
    };
    for action in actions.get_just_pressed() {
        match action {
            Controls::SystemPower { dir, system } => {
                power.send(AdjustPower { dir, system });
            }
            Controls::WeaponPower { dir, weapon_index } => {
                let Some(weapons) = &ship.basic.weapons else {
                    continue;
                };
                if weapon_index >= weapons.weapons.len() {
                    continue;
                }
                if weapons.weapons[weapon_index].powered && dir == PowerDir::Request {
                    commands.queue(start_targeting(weapon_index));
                } else {
                    weapon_power.send(WeaponPower { dir, weapon_index });
                }
            }
            Controls::Autofire => {
                set_autofire.send(SetAutofire(!self_intel.autofire));
            }
            Controls::AllDoors { open } => {
                set_doors_open.send(SetDoorsOpen::All { open });
            }
            Controls::SaveStations => {
                crew_stations.send(CrewStations::Save);
            }
            Controls::ReturnToStations => {
                crew_stations.send(CrewStations::Return);
            }
        }
    }
}
