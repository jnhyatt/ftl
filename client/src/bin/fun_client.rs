use bevy::{prelude::*, window::PrimaryWindow};
use bevy_egui::{egui, EguiContexts, EguiPlugin};
use bevy_mod_picking::prelude::*;
use client::{
    client_plugin,
    egui_common::{
        power_panel, ready_panel, shields_panel, status_panel, weapon_charge_ui, weapon_power_ui,
        weapon_rearrange_ui,
    },
    select::{selection_plugin, SelectEvent, Selectable, Selected, SelectionEnabled},
};
use common::{
    events::{
        AdjustPower, MoveWeapon, PowerDir, SetCrewGoal, SetProjectileWeaponTarget, WeaponPower,
    },
    intel::{SelfIntel, ShipIntel, WeaponChargeIntel},
    lobby::ReadyState,
    nav::{Cell, CrewNavStatus, LineSection, NavLocation, SquareSection},
    projectiles::{FiredFrom, RoomTarget, Traversal},
    ship::{Dead, SystemId},
};
use is_even::IsEven;
use leafwing_input_manager::{
    action_state::ActionState, input_map::InputMap, plugin::InputManagerPlugin, Actionlike,
    InputManagerBundle,
};
use rand::{thread_rng, Rng};
use std::f32::consts::TAU;

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
            client_plugin,
            selection_plugin,
        ))
        .insert_resource(Msaa::Off)
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                power_panel,
                status_panel,
                weapons_panel,
                shields_panel,
                ready_panel.run_if(resource_exists::<ReadyState>),
                add_ship_graphic,
                crew_panel,
            ),
        )
        .add_systems(Update, (sync_crew_count, sync_crew_positions).chain())
        .add_systems(Update, (spawn_projectile_graphics, update_bullet_graphic))
        .add_systems(Update, (controls, draw_targets))
        .run();
}

#[derive(Clone, Copy)]
enum CellTex {
    TopRight,
    Top,
    TopLeft,
    BottomLeft,
    Bottom,
    BottomRight,
}

#[derive(Component)]
struct CrewGraphic(usize);

// Switch this to use crew intel when own ship is intel-based
fn sync_crew_count(
    self_intel: Query<&SelfIntel>,
    crew: Query<(Entity, &Parent, &CrewGraphic)>,
    assets: Res<AssetServer>,
    mut commands: Commands,
) {
    let Ok(self_intel) = self_intel.get_single() else {
        return;
    };
    let crew_graphics = crew
        .iter()
        .filter(|&(_, parent, _)| **parent == self_intel.ship)
        .collect::<Vec<_>>();
    let crew_count = self_intel.crew.len();
    let crew_graphic_count = crew_graphics.len();
    for i in crew_count..crew_graphic_count {
        let e = crew_graphics.iter().find(|(_, _, x)| x.0 == i).unwrap().0;
        commands.entity(e).despawn();
    }
    for x in crew_graphic_count..crew_count {
        let new_crew_member = commands
            .spawn((
                CrewGraphic(x),
                Selectable { radius: 10.0 },
                Pickable {
                    should_block_lower: false,
                    is_hoverable: true,
                },
                SpriteBundle {
                    texture: assets.load("crew.png"),
                    transform: Transform::from_xyz(0.0, 0.0, 2.0),
                    ..default()
                },
            ))
            .id();
        commands.entity(self_intel.ship).add_child(new_crew_member);
    }
}

fn sync_crew_positions(
    self_intel: Query<&SelfIntel>,
    mut crew: Query<(&mut Transform, &Parent, &CrewGraphic)>,
) {
    let Ok(self_intel) = self_intel.get_single() else {
        return;
    };
    let mut crew_graphics = crew
        .iter_mut()
        .filter(|&(_, parent, _)| **parent == self_intel.ship)
        .collect::<Vec<_>>();
    crew_graphics.sort_unstable_by_key(|(_, _, x)| x.0);
    let crew = self_intel.crew.iter();
    let cell_pos = |cell: &Cell| match cell.0 {
        0 => Vec2::new(-52.5, -17.5),
        1 => Vec2::new(-17.5, -17.5),
        2 => Vec2::new(-52.5, 17.5),
        3 => Vec2::new(-17.5, 17.5),
        4 => Vec2::new(17.5, -17.5),
        5 => Vec2::new(17.5, 17.5),
        6 => Vec2::new(52.5, -17.5),
        7 => Vec2::new(52.5, 17.5),
        _ => unreachable!(),
    };
    for (crew, (mut graphic, _, _)) in crew.zip(crew_graphics) {
        let crew_z = graphic.translation.z;
        let crew_xy = match &crew.nav_status {
            CrewNavStatus::At(x) => cell_pos(x),
            CrewNavStatus::Navigating(x) => match &x.current_location {
                NavLocation::Line(LineSection([a, b]), x) => cell_pos(a).lerp(cell_pos(b), *x),
                NavLocation::Square(SquareSection([[a, b], [c, d]]), x) => {
                    let bottom = cell_pos(a).lerp(cell_pos(b), x.y);
                    let top = cell_pos(c).lerp(cell_pos(d), x.y);
                    bottom.lerp(top, x.x)
                }
            },
        };
        graphic.translation = crew_xy.extend(crew_z);
    }
}

fn cell_tex(assets: &AssetServer, x: CellTex) -> Handle<Image> {
    assets.load(match x {
        CellTex::TopRight => "cell-top-right.png",
        CellTex::Top => "cell-top.png",
        CellTex::TopLeft => "cell-top-left.png",
        CellTex::BottomLeft => "cell-bottom-left.png",
        CellTex::Bottom => "cell-bottom.png",
        CellTex::BottomRight => "cell-bottom-right.png",
    })
}

fn handle_cell_click(
    event: Listener<Pointer<Down>>,
    weapon: Option<Res<TargetingWeapon>>,
    self_intel: Query<&SelfIntel>,
    ships: Query<&ShipIntel>,
    cells: Query<(&RoomGraphic, &Parent)>,
    selected_crew: Query<&CrewGraphic, With<Selected>>,
    mut targeting: EventWriter<SetProjectileWeaponTarget>,
    mut set_crew_goal: EventWriter<SetCrewGoal>,
    mut commands: Commands,
) {
    let (&RoomGraphic(room), parent) = cells.get(event.target).unwrap();
    match event.button {
        PointerButton::Primary => {
            // Target selected weapon at this cell's room
            let Some(&TargetingWeapon(weapon_index)) = weapon.as_ref().map(|x| x.as_ref()) else {
                return;
            };
            let ship = **parent;
            let client_ship = self_intel.single().ship;
            if ship == client_ship {
                // If we're targeting self, make sure that's ok
                let client_intel = ships.get(client_ship).unwrap();
                let weapon =
                    &client_intel.basic.weapons.as_ref().unwrap().weapons[weapon_index].weapon;
                if !weapon.can_target_self {
                    return;
                }
            }
            targeting.send(SetProjectileWeaponTarget {
                target: Some(RoomTarget { ship, room }),
                weapon_index,
            });
            commands.remove_resource::<TargetingWeapon>();
            commands.insert_resource(SelectionEnabled);
        }
        PointerButton::Secondary => {
            // Send selected crew to this cell's room
            for &CrewGraphic(crew) in &selected_crew {
                set_crew_goal.send(SetCrewGoal { crew, room });
            }
        }
        _ => {}
    }
}

fn add_ship_graphic(
    self_intel: Query<&SelfIntel>,
    ships: Query<Entity, (With<ShipIntel>, Without<Sprite>)>,
    assets: Res<AssetServer>,
    mut commands: Commands,
) {
    let Ok(self_intel) = self_intel.get_single() else {
        return;
    };
    let my_ship = self_intel.ship;

    let cells = [
        (CellTex::BottomLeft, IVec2::new(0, 0), RoomGraphic(0)),
        (CellTex::BottomRight, IVec2::new(1, 0), RoomGraphic(0)),
        (CellTex::TopLeft, IVec2::new(0, 1), RoomGraphic(0)),
        (CellTex::TopRight, IVec2::new(1, 1), RoomGraphic(0)),
        (CellTex::Bottom, IVec2::new(2, 0), RoomGraphic(1)),
        (CellTex::Top, IVec2::new(2, 1), RoomGraphic(1)),
        (CellTex::Bottom, IVec2::new(3, 0), RoomGraphic(2)),
        (CellTex::Top, IVec2::new(3, 1), RoomGraphic(2)),
    ];
    let offset = cells.iter().fold(IVec2::ZERO, |sum, (_, x, _)| sum + *x);
    let bump_x = if offset.x.is_even() { 0.0 } else { 0.5 };
    let bump_y = if offset.y.is_even() { 0.0 } else { 0.5 };
    let pixel_offset = Vec2::new(bump_x, bump_y);
    let offset = offset.as_vec2() / cells.len() as f32;
    for ship in &ships {
        let transform = if ship == my_ship {
            Transform::from_xyz(-200.0, 0.0, -2.0)
        } else {
            Transform::from_xyz(400.0, 0.0, -2.0).with_rotation(Quat::from_rotation_z(TAU / 4.0))
        };

        commands.entity(ship).insert(SpriteBundle {
            texture: assets.load("potato-bug.png"),
            transform,
            ..default()
        });
        commands.entity(ship).with_children(|ship| {
            ship.spawn((
                Pickable::IGNORE,
                SpriteBundle {
                    transform: Transform::from_translation(delete_me(0).extend(1.5))
                        .with_rotation(transform.rotation.inverse()),
                    texture: assets.load("shields.png"),
                    ..default()
                },
            ));
            ship.spawn((
                Pickable::IGNORE,
                SpriteBundle {
                    transform: Transform::from_translation(delete_me(1).extend(1.5))
                        .with_rotation(transform.rotation.inverse()),
                    texture: assets.load("engines.png"),
                    ..default()
                },
            ));
            ship.spawn((
                Pickable::IGNORE,
                SpriteBundle {
                    transform: Transform::from_translation(delete_me(2).extend(1.5))
                        .with_rotation(transform.rotation.inverse()),
                    texture: assets.load("weapons.png"),
                    ..default()
                },
            ));
        });
        if ship == my_ship {
            use KeyCode::*;
            use SystemId::*;
            commands.entity(ship).insert(InputManagerBundle::with_map(
                InputMap::default()
                    .insert(Controls::power_system(Shields), KeyA)
                    .insert(Controls::power_system(Shields), KeyA)
                    .insert(Controls::power_system(Engines), KeyS)
                    .insert(Controls::power_system(Weapons), KeyW)
                    .insert(Controls::power_weapon(0), Digit1)
                    .insert(Controls::power_weapon(1), Digit2)
                    .insert(Controls::power_weapon(2), Digit3)
                    .insert(Controls::power_weapon(3), Digit4)
                    .insert_chord(Controls::depower_system(Shields), [ShiftLeft, KeyA])
                    .insert_chord(Controls::depower_system(Engines), [ShiftLeft, KeyS])
                    .insert_chord(Controls::depower_system(Weapons), [ShiftLeft, KeyW])
                    .insert_chord(Controls::depower_weapon(0), [ShiftLeft, Digit1])
                    .insert_chord(Controls::depower_weapon(1), [ShiftLeft, Digit2])
                    .insert_chord(Controls::depower_weapon(2), [ShiftLeft, Digit3])
                    .insert_chord(Controls::depower_weapon(3), [ShiftLeft, Digit4])
                    .build(),
            ));
        }
        for (tex, pos, room) in cells.iter().cloned() {
            let pos = (pos.as_vec2() - offset) * 35.0 + pixel_offset;
            let cell = commands
                .spawn((
                    On::<Pointer<Down>>::run(handle_cell_click),
                    room,
                    SpriteBundle {
                        texture: cell_tex(&*assets, tex),
                        transform: Transform::from_translation(pos.extend(1.0)),
                        ..default()
                    },
                ))
                .id();
            commands.entity(ship).add_child(cell);
        }
    }
}

#[derive(Component, Clone, Copy)]
struct RoomGraphic(usize);

fn setup(mut commands: Commands, assets: Res<AssetServer>) {
    commands.spawn(Camera2dBundle::default());
    commands.spawn(SpriteBundle {
        texture: assets.load("background-1.png"),
        transform: Transform::from_xyz(0.0, 0.0, -3.0),
        ..default()
    });
    commands.spawn((
        On::<Pointer<Down>>::send_event::<SelectEvent>(),
        On::<Pointer<Up>>::send_event::<SelectEvent>(),
        On::<Pointer<Drag>>::send_event::<SelectEvent>(),
        Pickable {
            should_block_lower: false,
            is_hoverable: true,
        },
        NodeBundle {
            style: Style {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            ..default()
        },
    ));

    commands.spawn((
        On::<Pointer<Down>>::run(
            |event: Listener<Pointer<Down>>,
             weapon: Option<Res<TargetingWeapon>>,
             mut targeting: EventWriter<SetProjectileWeaponTarget>,
             mut commands: Commands| {
                let Some(weapon) = weapon else {
                    return;
                };
                if event.button == PointerButton::Secondary {
                    targeting.send(SetProjectileWeaponTarget {
                        target: None,
                        weapon_index: weapon.0,
                    });
                    commands.remove_resource::<TargetingWeapon>();
                    commands.init_resource::<SelectionEnabled>();
                }
            },
        ),
        Pickable {
            should_block_lower: false,
            is_hoverable: true,
        },
        NodeBundle {
            style: Style {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            ..default()
        },
    ));
}

#[derive(Resource, Debug)]
struct TargetingWeapon(usize);

fn weapons_panel(
    mut ui: EguiContexts,
    self_intel: Query<&SelfIntel>,
    ships: Query<&ShipIntel, Without<Dead>>,
    charge_intel: Query<&WeaponChargeIntel>,
    mut targeting: EventWriter<SetProjectileWeaponTarget>,
    mut weapon_power: EventWriter<WeaponPower>,
    mut weapon_ordering: EventWriter<MoveWeapon>,
    mut commands: Commands,
) {
    let Ok(self_intel) = self_intel.get_single() else {
        // No connection to server
        return;
    };
    let Ok(intel) = ships.get(self_intel.ship) else {
        // Ship destroyed
        return;
    };
    let Some(weapons) = &intel.basic.weapons else {
        // No weapons system
        return;
    };
    let weapon_charges = charge_intel.get(intel.weapon_charge).unwrap();
    egui::Window::new("Weapons").show(ui.ctx_mut(), |ui| {
        let last_weapon = weapons.weapons.len() - 1;
        for (weapon_index, weapon) in weapons.weapons.iter().enumerate() {
            ui.horizontal(|ui| {
                weapon_rearrange_ui(ui, weapon_index, last_weapon, &mut weapon_ordering);
                weapon_power_ui(
                    ui,
                    weapon.powered,
                    weapon_index,
                    &weapon.weapon,
                    &mut weapon_power,
                );
                ui.label(weapon.weapon.name);
                weapon_charge_ui(ui, weapon_charges.levels[weapon_index], &weapon.weapon);

                if ui.button("Target").clicked() {
                    // Disable selection, target weapon `index`
                    targeting.send(SetProjectileWeaponTarget {
                        weapon_index,
                        target: None,
                    });
                    commands.insert_resource(TargetingWeapon(weapon_index));
                    commands.remove_resource::<SelectionEnabled>();
                }
            });
        }
    });
}

fn crew_panel(mut ui: EguiContexts, self_intel: Query<&SelfIntel>) {
    let Ok(self_intel) = self_intel.get_single() else {
        return;
    };
    egui::Window::new("Crew").show(ui.ctx_mut(), |ui| {
        for (_crew_index, crew) in self_intel.crew.iter().enumerate() {
            ui.group(|ui| {
                ui.heading(&crew.name);
                ui.label(format!(
                    "Health: {}/{}",
                    crew.health as usize, crew.max_health as usize
                ));
            });
        }
    });
}

#[derive(Component, Deref)]
struct BulletIncidence(Direction2d);

fn spawn_projectile_graphics(
    bullets: Query<Entity, (With<Traversal>, Without<Sprite>)>,
    assets: Res<AssetServer>,
    mut commands: Commands,
) {
    for bullet in &bullets {
        let direction =
            Direction2d::new_unchecked(Vec2::from_angle(thread_rng().gen_range(0.0..=TAU)));
        commands.entity(bullet).insert((
            SpriteBundle {
                texture: assets.load("missile-1.png"),
                ..default()
            },
            BulletIncidence(direction),
        ));
    }
}

fn update_bullet_graphic(
    ships: Query<&Transform, Without<Traversal>>,
    mut bullets: Query<(
        &Traversal,
        &RoomTarget,
        &FiredFrom,
        &BulletIncidence,
        &mut Transform,
    )>,
) {
    for (traversal, target, origin, incidence, mut bullet) in &mut bullets {
        let target_transform = ships.get(target.ship).unwrap();
        let origin = ships.get(origin.ship).unwrap().translation.xy(); // TODO weapon mount
        let out_mid = Vec2::X * 1000.0;
        let destination = (target_transform.rotation * delete_me(target.room).extend(3.0)
            + target_transform.translation)
            .xy(); // TODO room
        let in_mid = destination - 1000.0 * ***incidence;

        bullet.translation = if **traversal < 0.5 {
            origin.lerp(out_mid, **traversal * 2.0)
        } else {
            in_mid.lerp(destination, **traversal * 2.0 - 1.0)
        }
        .extend(3.0);
        bullet.rotation = if **traversal < 0.5 {
            Quat::IDENTITY
        } else {
            Quat::from_rotation_arc_2d(Vec2::X, ***incidence)
        };
    }
}

#[derive(Actionlike, Reflect, Clone, Hash, PartialEq, Eq)]
enum Controls {
    SystemPower { dir: PowerDir, system: SystemId },
    WeaponPower { dir: PowerDir, weapon_index: usize },
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
    ships: Query<(&ShipIntel, &ActionState<Controls>)>,
    mut targeting: EventWriter<SetProjectileWeaponTarget>,
    mut power: EventWriter<AdjustPower>,
    mut weapon_power: EventWriter<WeaponPower>,
    mut commands: Commands,
) {
    for (ship, actions) in &ships {
        for action in actions.get_just_pressed() {
            match action {
                Controls::SystemPower { dir, system } => {
                    power.send(AdjustPower { dir, system });
                }
                Controls::WeaponPower { dir, weapon_index } => {
                    let Some(weapons) = &ship.basic.weapons else {
                        continue;
                    };
                    if weapons.weapons[weapon_index].powered && dir == PowerDir::Request {
                        targeting.send(SetProjectileWeaponTarget {
                            weapon_index,
                            target: None,
                        });
                        commands.insert_resource(TargetingWeapon(weapon_index));
                    } else {
                        weapon_power.send(WeaponPower { dir, weapon_index });
                    }
                }
            }
        }
    }
}

fn draw_targets(
    windows: Query<&Window, With<PrimaryWindow>>,
    self_intel: Query<&SelfIntel>,
    ships: Query<&Transform>,
    targeting_weapon: Option<Res<TargetingWeapon>>,
    mut gizmos: Gizmos,
) {
    let Ok(self_intel) = self_intel.get_single() else {
        return;
    };
    let size_color = |index| match index {
        0 => (24.0, Color::RED),
        1 => (28.0, Color::ORANGE_RED),
        2 => (32.0, Color::ORANGE),
        3 => (36.0, Color::YELLOW),
        _ => unreachable!(),
    };
    if let Some(cursor) = windows.get_single().ok().and_then(|x| x.cursor_position()) {
        let world_cursor = cursor * Vec2::new(1.0, -1.0) + Vec2::new(-640.0, 360.0);
        if let Some(targeting) = targeting_weapon {
            let (size, color) = size_color(targeting.0);
            gizmos.circle(world_cursor.extend(5.0), Direction3d::Z, size, color);
        }
    }

    for (i, target) in self_intel.weapon_targets.iter().enumerate() {
        if let Some(target) = target {
            let target_transform = ships.get(target.ship).unwrap();
            let room_location = delete_me(target.room).extend(2.0);
            let pos = target_transform.rotation * room_location + target_transform.translation;
            let (size, color) = size_color(i);
            gizmos.circle(pos, Direction3d::Z, size, color);
        }
    }
}

fn delete_me(room: usize) -> Vec2 {
    match room {
        0 => Vec2::new(-35.0, 0.0),
        1 => Vec2::new(17.5, 0.0),
        2 => Vec2::new(52.5, 0.0),
        _ => unreachable!(),
    }
}
