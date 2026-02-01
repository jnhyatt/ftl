mod egui_panels;
mod graphics;
mod pointer;
mod selection;
mod targeting;

use crate::{
    egui_panels::{
        crew_panel, enemy_panels, power_panel, ready_panel, shields_panel, status_panel,
        weapons_panel,
    },
    graphics::graphics_plugin,
    pointer::{
        pointer_plugin,
        selection::{finish_selection, grow_selection, start_selection},
        targeting::{
            aim_beam, cancel_targeting, DisableWhenTargeting, EnableWhenTargeting, TargetingWeapon,
        },
    },
    selection::selection_plugin,
};
use bevy::{
    prelude::*,
    remote::{http::RemoteHttpPlugin, RemotePlugin},
};
use bevy_egui::{EguiPlugin, EguiPrimaryContextPass};
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
    util::{disable_observer, enable_observer, DisabledObserver},
    PROTOCOL_ID,
};
use graphics::{
    add_ship_graphic, draw_beams, draw_targets, set_bullet_incidence, spawn_projectile_graphics,
    sync_door_sprites, sync_no_intel_overlays, sync_oxygen_overlays, sync_vacuum_overlays,
    update_bullet_graphic,
};
use leafwing_input_manager::{
    action_state::ActionState,
    input_map::InputMap,
    plugin::InputManagerPlugin,
    prelude::{ButtonlikeChord, ModifierKey},
    Actionlike, InputControlKind,
};
use std::{
    net::{Ipv4Addr, SocketAddr, UdpSocket},
    time::SystemTime,
};
use targeting::start_targeting;

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(window()),
                    ..default()
                })
                .set(ImagePlugin::default_nearest()),
            #[cfg(debug_assertions)]
            (RemotePlugin::default(), RemoteHttpPlugin::default()),
            EguiPlugin::default(),
            InputManagerPlugin::<Controls>::default(),
            (RepliconPlugins, RepliconRenetPlugins),
            protocol_plugin,
            selection_plugin,
            graphics_plugin,
            pointer_plugin,
        ))
        .insert_resource(SpritePickingSettings {
            picking_mode: SpritePickingMode::BoundingBox,
            ..default()
        })
        .add_systems(Startup, (setup, connect_to_server))
        .add_systems(
            EguiPrimaryContextPass,
            (
                power_panel,
                status_panel,
                weapons_panel,
                shields_panel,
                enemy_panels,
                crew_panel,
                ready_panel.run_if(resource_exists::<ReadyState>),
            )
                .run_if(any_with_component::<SelfIntel>),
        )
        .add_systems(
            Update,
            (
                set_bullet_incidence,
                spawn_projectile_graphics,
                update_bullet_graphic,
                draw_beams,
                sync_door_sprites,
                sync_oxygen_overlays,
                sync_vacuum_overlays,
                sync_no_intel_overlays.run_if(any_with_component::<SelfIntel>),
            ),
        )
        .add_systems(
            Update,
            (controls, draw_targets, add_ship_controls, add_ship_graphic)
                .run_if(any_with_component::<SelfIntel>),
        )
        .add_systems(
            Update,
            (
                on_finish_targeting.run_if(resource_removed::<TargetingWeapon>),
                on_start_targeting.run_if(resource_added::<TargetingWeapon>),
            ),
        )
        .run();
}

fn window() -> Window {
    Window {
        resolution: bevy::window::WindowResolution::new(1280, 720),
        title: "PVP: Paster Vhan Pight".into(),
        resizable: false,
        enabled_buttons: bevy::window::EnabledButtons {
            maximize: false,
            ..default()
        },
        ..default()
    }
}

fn connect_to_server(channels: Res<RepliconChannels>, mut commands: Commands) {
    let current_time = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap();
    let client_id = current_time.as_millis() as u64;
    let server_addr = SocketAddr::new(Ipv4Addr::new(127, 0, 0, 1).into(), 5000);
    let socket = UdpSocket::bind(SocketAddr::from((Ipv4Addr::UNSPECIFIED, 0))).unwrap();
    let authentication = ClientAuthentication::Unsecure {
        client_id,
        protocol_id: PROTOCOL_ID,
        server_addr,
        user_data: None,
    };
    commands.insert_resource(RenetClient::new(ConnectionConfig {
        server_channels_config: channels.server_configs(),
        client_channels_config: channels.client_configs(),
        ..default()
    }));
    commands.insert_resource(
        NetcodeClientTransport::new(current_time, authentication, socket).unwrap(),
    );
}

fn setup(assets: Res<AssetServer>, mut commands: Commands) {
    // Lots of sprites have x/y values that have 0 fractional part, and that can make them a little
    // temperamental in terms of which pixels they decide to occupy. If we shift the camera just a
    // quarter pixel up and right, this resolves all issues with these sprites by putting their
    // texels solidly on a pixel, rather than right on the border.
    commands.spawn((
        Camera2d,
        Name::new("Camera"),
        Transform::from_xyz(0.25, 0.25, 0.0),
    ));
    commands.spawn((
        Name::new("Background"),
        Sprite {
            image: assets.load("background-1.png"),
            ..default()
        },
    ));
    // Spawn in the foreground click target
    let foreground_click_plane = commands
        .spawn((
            Name::new("Screen Quad"),
            Sprite {
                color: Color::WHITE.with_alpha(0.0),
                custom_size: Some(Vec2::new(1280.0, 720.0)),
                ..default()
            },
            Pickable {
                should_block_lower: false,
                is_hoverable: true,
            },
            // This puts the click plane in front of everything else. Unfortunately the UI picking
            // backend doesn't give us hit position, so we have to use a sprite instead. Otherwise
            // this wouldn't be a problem. Note 1000 is the near plane.
            Transform::from_xyz(0.0, 0.0, 999.0),
        ))
        .observe(cancel_targeting)
        .id();
    commands.spawn((
        Observer::new(start_selection).with_entity(foreground_click_plane),
        DisableWhenTargeting,
    ));
    commands.spawn((
        Observer::new(grow_selection).with_entity(foreground_click_plane),
        DisableWhenTargeting,
    ));
    commands.spawn((
        Observer::new(finish_selection).with_entity(foreground_click_plane),
        DisableWhenTargeting,
    ));
    commands.spawn((
        DisabledObserver(Observer::new(aim_beam).with_entity(foreground_click_plane)),
        EnableWhenTargeting,
    ));
}

fn on_start_targeting(
    to_disable: Query<Entity, With<DisableWhenTargeting>>,
    to_enable: Query<Entity, With<EnableWhenTargeting>>,
    mut commands: Commands,
) {
    for observer in &to_disable {
        commands.entity(observer).queue(disable_observer);
    }
    for observer in &to_enable {
        commands.entity(observer).queue(enable_observer);
    }
}

fn on_finish_targeting(
    to_enable: Query<Entity, With<DisableWhenTargeting>>,
    to_disable: Query<Entity, With<EnableWhenTargeting>>,
    mut commands: Commands,
) {
    for observer in &to_enable {
        commands.entity(observer).queue(enable_observer);
    }
    for observer in &to_disable {
        commands.entity(observer).queue(disable_observer);
    }
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
        commands.entity(ship).insert(input_map);
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
    self_intel: Single<&SelfIntel>,
    ships: Query<(&ShipIntel, &ActionState<Controls>)>,
    mut power: MessageWriter<AdjustPower>,
    mut weapon_power: MessageWriter<WeaponPower>,
    mut set_autofire: MessageWriter<SetAutofire>,
    mut set_doors_open: MessageWriter<SetDoorsOpen>,
    mut crew_stations: MessageWriter<CrewStations>,
    mut commands: Commands,
) {
    let Ok((ship, actions)) = ships.get(self_intel.ship) else {
        return;
    };
    for action in actions.get_just_pressed() {
        match action {
            Controls::SystemPower { dir, system } => {
                power.write(AdjustPower { dir, system });
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
                    weapon_power.write(WeaponPower { dir, weapon_index });
                }
            }
            Controls::Autofire => {
                set_autofire.write(SetAutofire(!self_intel.autofire));
            }
            Controls::AllDoors { open } => {
                set_doors_open.write(SetDoorsOpen::All { open });
            }
            Controls::SaveStations => {
                crew_stations.write(CrewStations::Save);
            }
            Controls::ReturnToStations => {
                crew_stations.write(CrewStations::Return);
            }
        }
    }
}
