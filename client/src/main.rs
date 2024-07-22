use bevy::{prelude::*, window::PrimaryWindow};
use bevy_egui::{egui, EguiContexts, EguiPlugin};
use bevy_mod_picking::prelude::*;
use client::{
    client_plugin,
    egui_common::{
        enemy_panels, power_panel, ready_panel, shields_panel, status_panel, weapon_charge_ui,
        weapon_power_ui, weapon_rearrange_ui,
    },
    select::{selection_plugin, SelectEvent, Selectable, Selected, SelectionEnabled},
};
use common::{
    events::{
        AdjustPower, MoveWeapon, PowerDir, SetAutofire, SetCrewGoal, SetDoorsOpen,
        SetProjectileWeaponTarget, WeaponPower,
    },
    intel::{InteriorIntel, SelfIntel, ShipIntel, WeaponChargeIntel},
    lobby::ReadyState,
    nav::{Cell, CrewNavStatus, LineSection, NavLocation, SquareSection},
    projectiles::{FiredFrom, RoomTarget, Traversal},
    ship::{Dead, Door, DoorDir, SystemId, SHIPS},
    util::round_to_usize,
    RACES,
};
use leafwing_input_manager::{
    action_state::ActionState, input_map::InputMap, plugin::InputManagerPlugin, Actionlike,
    InputManagerBundle,
};
use rand::{thread_rng, Rng};
use std::f32::consts::TAU;

const Z_BG: f32 = 0.0;
const Z_SHIP: f32 = Z_BG + 1.0;
const Z_BULLETS: f32 = Z_SHIP + Z_SHIELDS + 1.0;

const Z_CELL: f32 = 1.0;
const Z_ICONS: f32 = 4.0;
const Z_CREW: f32 = Z_ICONS + 1.0;
const Z_SHIELDS: f32 = Z_CREW + 1.0;

const Z_AIR: f32 = 1.0;
const Z_VACUUM: f32 = Z_AIR + 1.0;
const Z_WALLS: f32 = Z_VACUUM + 1.0;

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
                enemy_panels,
                ready_panel.run_if(resource_exists::<ReadyState>),
                add_ship_graphic,
                crew_panel,
            ),
        )
        .add_systems(Update, (sync_crew_count, sync_crew_positions).chain())
        .add_systems(
            Update,
            (
                spawn_projectile_graphics,
                update_bullet_graphic,
                update_doors,
                update_oxygen,
                update_vacuum,
            ),
        )
        .add_systems(Update, (controls, draw_targets))
        .run();
}

#[derive(Clone, Copy)]
enum Walls {
    TopRight,
    Top,
    TopLeft,
    Left,
    BottomLeft,
    Bottom,
    BottomRight,
    Right,
}

#[derive(Component)]
struct DoorGraphic(usize);

#[derive(Component)]
struct CrewGraphic(usize);

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
                    transform: Transform::from_xyz(0.0, 0.0, Z_CREW),
                    ..default()
                },
            ))
            .id();
        commands.entity(self_intel.ship).add_child(new_crew_member);
    }
}

fn sync_crew_positions(
    self_intel: Query<&SelfIntel>,
    ships: Query<&ShipIntel>,
    mut crew: Query<(&mut Transform, &Parent, &CrewGraphic)>,
) {
    let Ok(self_intel) = self_intel.get_single() else {
        return;
    };
    let ship = &SHIPS[ships.get(self_intel.ship).unwrap().basic.ship_type];
    let mut crew_graphics = crew
        .iter_mut()
        .filter(|&(_, parent, _)| **parent == self_intel.ship)
        .collect::<Vec<_>>();
    crew_graphics.sort_unstable_by_key(|(_, _, x)| x.0);
    let crew = self_intel.crew.iter();
    let cell_pos = |&Cell(cell)| ship.cell_positions[cell];
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

fn walls_tex(assets: &AssetServer, x: Walls) -> Handle<Image> {
    assets.load(match x {
        Walls::TopRight => "walls-corner.png",
        Walls::TopLeft => "walls-corner.png",
        Walls::BottomLeft => "walls-corner.png",
        Walls::BottomRight => "walls-corner.png",
        Walls::Top => "walls-edge.png",
        Walls::Left => "walls-edge.png",
        Walls::Bottom => "walls-edge.png",
        Walls::Right => "walls-edge.png",
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

#[derive(Bundle)]
pub struct DoorBundle {
    door: DoorGraphic,
    listener: On<Pointer<Click>>,
    sprite: SpriteBundle,
}

impl DoorBundle {
    fn new(ship_type: usize, index: usize) -> Self {
        let ship = &SHIPS[ship_type];
        let cells = ship.cell_positions;
        let door_pos = match ship.doors[index] {
            Door::Interior(a, b) => (cells[a.0] + cells[b.0]) / 2.0,
            Door::Exterior(cell, dir) => cells[cell.0] + dir.offset(),
        };
        let normal = match ship.doors[index] {
            Door::Interior(a, b) => cells[b.0] - cells[a.0],
            Door::Exterior(_, dir) => dir.offset(),
        }
        .normalize_or_zero();
        let door_pos = (door_pos).extend(Z_CREW);
        let transform =
            Transform::from_translation(door_pos).with_rotation(Quat::from_mat3(&Mat3 {
                x_axis: normal.extend(0.0),
                y_axis: normal.perp().extend(0.0),
                z_axis: Vec3::Z,
            }));
        Self {
            door: DoorGraphic(index),
            listener: On::<Pointer<Click>>::run(Self::toggle_door),
            sprite: SpriteBundle {
                transform,
                ..default()
            },
        }
    }

    fn toggle_door(
        event: Listener<Pointer<Click>>,
        ships: Query<&ShipIntel, Without<Dead>>,
        doors: Query<(&DoorGraphic, &Parent)>,
        mut set_doors_open: EventWriter<SetDoorsOpen>,
    ) {
        let (&DoorGraphic(door), parent) = doors.get(event.target).unwrap();
        let Ok(ship) = ships.get(**parent) else {
            return;
        };
        let is_open = ship.basic.doors[door].open;
        set_doors_open.send(SetDoorsOpen::Single {
            door,
            open: !is_open,
        });
    }
}

fn add_ship_graphic(
    self_intel: Query<&SelfIntel>,
    ships: Query<(Entity, &ShipIntel), Without<Sprite>>,
    assets: Res<AssetServer>,
    mut commands: Commands,
) {
    let Ok(self_intel) = self_intel.get_single() else {
        return;
    };
    let my_ship = self_intel.ship;
    for (ship, intel) in &ships {
        let transform = if ship == my_ship {
            Transform::from_xyz(-200.0, 0.0, Z_SHIP)
        } else {
            Transform::from_xyz(400.0, 0.0, Z_SHIP).with_rotation(Quat::from_rotation_z(TAU / 4.0))
        };

        commands.entity(ship).insert(SpriteBundle {
            texture: assets.load("potato-bug.png"),
            transform,
            ..default()
        });

        let icon = |system| {
            let sprite = match system {
                SystemId::Engines => "engines.png",
                SystemId::Shields => "shields.png",
                SystemId::Weapons => "weapons.png",
                SystemId::Oxygen => "oxygen.png",
            };
            let room = SHIPS[intel.basic.ship_type]
                .room_systems
                .iter()
                .position(|x| *x == Some(system));
            room.map(|room| {
                (
                    Pickable::IGNORE,
                    SpriteBundle {
                        transform: Transform::from_translation(
                            room_center(intel, room).extend(Z_ICONS),
                        )
                        .with_rotation(transform.rotation.inverse()),
                        texture: assets.load(sprite),
                        ..default()
                    },
                )
            })
        };

        commands.entity(ship).with_children(|ship| {
            icon(SystemId::Shields).map(|x| ship.spawn(x));
            icon(SystemId::Engines).map(|x| ship.spawn(x));
            icon(SystemId::Weapons).map(|x| ship.spawn(x));
            icon(SystemId::Oxygen).map(|x| ship.spawn(x));
        });

        if ship == my_ship {
            use KeyCode::*;
            use SystemId::*;
            commands
                .entity(ship)
                .insert(InputManagerBundle::with_map(
                    InputMap::default()
                        .insert(Controls::Autofire, KeyV)
                        .insert(Controls::AllDoors { open: true }, KeyZ)
                        .insert(Controls::AllDoors { open: false }, KeyX)
                        .insert(Controls::SetStations, Slash)
                        .insert(Controls::GoStations, Enter)
                        .insert(Controls::power_system(Shields), KeyA)
                        .insert(Controls::power_system(Engines), KeyS)
                        .insert(Controls::power_system(Weapons), KeyW)
                        .insert(Controls::power_system(Oxygen), KeyF)
                        .insert(Controls::power_weapon(0), Digit1)
                        .insert(Controls::power_weapon(1), Digit2)
                        .insert(Controls::power_weapon(2), Digit3)
                        .insert(Controls::power_weapon(3), Digit4)
                        .insert_chord(Controls::depower_system(Shields), [ShiftLeft, KeyA])
                        .insert_chord(Controls::depower_system(Engines), [ShiftLeft, KeyS])
                        .insert_chord(Controls::depower_system(Weapons), [ShiftLeft, KeyW])
                        .insert_chord(Controls::depower_system(Oxygen), [ShiftLeft, KeyF])
                        .insert_chord(Controls::depower_weapon(0), [ShiftLeft, Digit1])
                        .insert_chord(Controls::depower_weapon(1), [ShiftLeft, Digit2])
                        .insert_chord(Controls::depower_weapon(2), [ShiftLeft, Digit3])
                        .insert_chord(Controls::depower_weapon(3), [ShiftLeft, Digit4])
                        .build(),
                ))
                .with_children(|ship| {
                    for i in 0..SHIPS[intel.basic.ship_type].doors.len() {
                        ship.spawn(DoorBundle::new(intel.basic.ship_type, i));
                    }
                });
        }

        for (room_index, room) in SHIPS[intel.basic.ship_type].rooms.iter().enumerate() {
            let room_center = SHIPS[intel.basic.ship_type].room_center(room_index);
            for &Cell(cell) in room.cells {
                let cells = &SHIPS[intel.basic.ship_type].cell_positions;
                let tex = match (
                    cells[cell].x.total_cmp(&room_center.x),
                    cells[cell].y.total_cmp(&room_center.y),
                ) {
                    (std::cmp::Ordering::Less, std::cmp::Ordering::Less) => Walls::BottomLeft,
                    (std::cmp::Ordering::Less, std::cmp::Ordering::Equal) => Walls::Left,
                    (std::cmp::Ordering::Less, std::cmp::Ordering::Greater) => Walls::TopLeft,
                    (std::cmp::Ordering::Equal, std::cmp::Ordering::Less) => Walls::Bottom,
                    (std::cmp::Ordering::Equal, std::cmp::Ordering::Greater) => Walls::Top,
                    (std::cmp::Ordering::Greater, std::cmp::Ordering::Less) => Walls::BottomRight,
                    (std::cmp::Ordering::Greater, std::cmp::Ordering::Equal) => Walls::Right,
                    (std::cmp::Ordering::Greater, std::cmp::Ordering::Greater) => Walls::TopRight,
                    (std::cmp::Ordering::Equal, std::cmp::Ordering::Equal) => {
                        panic!("No center tiles")
                    }
                };
                let wall_rotation = match tex {
                    Walls::TopRight => Quat::from_rotation_z(TAU * 0.5),
                    Walls::Top => Quat::from_rotation_z(TAU * 0.5),
                    Walls::TopLeft => Quat::from_rotation_z(TAU * 0.75),
                    Walls::Left => Quat::from_rotation_z(TAU * 0.75),
                    Walls::BottomLeft => Quat::from_rotation_z(TAU * 0.0),
                    Walls::Bottom => Quat::from_rotation_z(TAU * 0.0),
                    Walls::BottomRight => Quat::from_rotation_z(TAU * 0.25),
                    Walls::Right => Quat::from_rotation_z(TAU * 0.25),
                };
                let wall_caps: &[DoorDir] = match tex {
                    Walls::TopRight => &[DoorDir::Right, DoorDir::Top],
                    Walls::Top => &[DoorDir::Right, DoorDir::Top, DoorDir::Left],
                    Walls::TopLeft => &[DoorDir::Top, DoorDir::Left],
                    Walls::Left => &[DoorDir::Top, DoorDir::Left, DoorDir::Bottom],
                    Walls::BottomLeft => &[DoorDir::Left, DoorDir::Bottom],
                    Walls::Bottom => &[DoorDir::Left, DoorDir::Bottom, DoorDir::Right],
                    Walls::BottomRight => &[DoorDir::Bottom, DoorDir::Right],
                    Walls::Right => &[DoorDir::Bottom, DoorDir::Right, DoorDir::Top],
                };
                let cell_graphic = commands
                    .spawn((
                        On::<Pointer<Down>>::run(handle_cell_click),
                        RoomGraphic(room_index),
                        SpriteBundle {
                            texture: assets.load("cell.png"),
                            transform: Transform::from_translation(cells[cell].extend(Z_CELL)),
                            ..default()
                        },
                    ))
                    .id();
                let oxygen = commands
                    .spawn((
                        Pickable::IGNORE,
                        OxygenGraphic(room_index),
                        SpriteBundle {
                            texture: assets.load("low-oxygen.png"),
                            transform: Transform::from_xyz(0.0, 0.0, Z_AIR),
                            ..default()
                        },
                    ))
                    .id();
                let vacuum = commands
                    .spawn((
                        Pickable::IGNORE,
                        VacuumGraphic(room_index),
                        SpriteBundle {
                            texture: assets.load("vacuum.png"),
                            transform: Transform::from_xyz(0.0, 0.0, Z_VACUUM),
                            ..default()
                        },
                    ))
                    .id();
                let walls = commands
                    .spawn((
                        Pickable::IGNORE,
                        SpriteBundle {
                            texture: walls_tex(assets.as_ref(), tex),
                            transform: Transform::from_xyz(0.0, 0.0, Z_WALLS)
                                .with_rotation(wall_rotation),
                            ..default()
                        },
                    ))
                    .id();

                let door_positions = SHIPS[intel.basic.ship_type]
                    .doors
                    .iter()
                    .map(|x| match x {
                        Door::Interior(a, b) => (cells[a.0] + cells[b.0]) / 2.0,
                        Door::Exterior(cell, dir) => cells[cell.0] + dir.offset(),
                    })
                    .collect::<Vec<_>>();
                for &cap in wall_caps {
                    if !door_positions.contains(&(cells[cell] + cap.offset())) {
                        let rotation = match cap {
                            DoorDir::Right => Quat::from_rotation_z(TAU * 0.0),
                            DoorDir::Top => Quat::from_rotation_z(TAU * 0.25),
                            DoorDir::Left => Quat::from_rotation_z(TAU * 0.5),
                            DoorDir::Bottom => Quat::from_rotation_z(TAU * 0.75),
                        };
                        let cap = commands
                            .spawn((
                                Pickable::IGNORE,
                                SpriteBundle {
                                    texture: assets.load("wall-cap.png"),
                                    transform: Transform::from_translation(
                                        cap.offset().extend(Z_WALLS),
                                    )
                                    .with_rotation(rotation),
                                    ..default()
                                },
                            ))
                            .id();
                        commands.entity(cell_graphic).add_child(cap);
                    }
                }
                commands.entity(ship).add_child(cell_graphic);
                commands.entity(cell_graphic).add_child(oxygen);
                commands.entity(cell_graphic).add_child(vacuum);
                commands.entity(cell_graphic).add_child(walls);
            }
        }
    }
}

fn update_doors(
    ships: Query<&ShipIntel>,
    mut doors: Query<(&DoorGraphic, &Parent, &mut Handle<Image>)>,
    assets: Res<AssetServer>,
) {
    for (&DoorGraphic(door), parent, mut sprite) in &mut doors {
        let Ok(ship) = ships.get(parent.get()) else {
            return;
        };
        let door = ship.basic.doors[door];
        *sprite = match door.open {
            _ if door.broken() => assets.load("door-broken.png"),
            false => assets.load("door-closed.png"),
            true => assets.load("door-open.png"),
        };
    }
}

fn update_oxygen(
    ships: Query<&ShipIntel, Without<Dead>>,
    interiors: Query<&InteriorIntel>,
    cells: Query<&Parent>,
    mut oxygen: Query<(&OxygenGraphic, &Parent, &mut Sprite)>,
) {
    for (&OxygenGraphic(room), parent, mut sprite) in &mut oxygen {
        let ship = **cells.get(**parent).unwrap();
        let Ok(ship) = ships.get(ship) else {
            continue;
        };
        let Ok(interior) = interiors.get(ship.interior) else {
            continue;
        };
        sprite.color.set_a(1.0 - interior.rooms[room].oxygen);
    }
}

fn update_vacuum(
    ships: Query<&ShipIntel, Without<Dead>>,
    interiors: Query<&InteriorIntel>,
    cells: Query<&Parent>,
    mut oxygen: Query<(&VacuumGraphic, &Parent, &mut Visibility)>,
) {
    for (&VacuumGraphic(room), parent, mut visibility) in &mut oxygen {
        let ship = **cells.get(**parent).unwrap();
        let Ok(ship) = ships.get(ship) else {
            continue;
        };
        let Ok(interior) = interiors.get(ship.interior) else {
            continue;
        };
        *visibility = if interior.rooms[room].oxygen < 0.05 {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

#[derive(Component, Clone, Copy)]
struct RoomGraphic(usize);

#[derive(Component, Clone, Copy)]
struct OxygenGraphic(usize);

#[derive(Component, Clone, Copy)]
struct VacuumGraphic(usize);

fn setup(mut commands: Commands, assets: Res<AssetServer>) {
    // Lots of sprites have x/y values that have 0 fractional part, and that can make them a little
    // temperamental in terms of which pixels they decide to occupy. If we shift the camera just a
    // quarter pixel up and right, this resolves all issues with these sprites by putting their
    // texels solidly on a pixel, rather than right on the border.
    commands.spawn(Camera2dBundle {
        transform: Transform::from_xyz(0.25, 0.25, 0.0),
        ..default()
    });
    commands.spawn(SpriteBundle {
        texture: assets.load("background-1.png"),
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
    mut set_autofire: EventWriter<SetAutofire>,
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
                ui.label(format!("[{}] {}", weapon_index + 1, weapon.weapon.name));
                weapon_charge_ui(ui, weapon_charges.levels[weapon_index], &weapon.weapon);
                let target_text = if let Some(target) = &self_intel.weapon_targets[weapon_index] {
                    format!("Target: {:?}", target)
                } else {
                    "Target".into()
                };
                if ui.button(target_text).clicked() {
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
        let mut autofire = self_intel.autofire;
        ui.checkbox(&mut autofire, "[V] Autofire");
        if autofire != self_intel.autofire {
            set_autofire.send(SetAutofire(autofire));
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
                    round_to_usize(crew.health),
                    round_to_usize(RACES[crew.race].max_health)
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
            Pickable::IGNORE,
            BulletIncidence(direction),
            SpriteBundle {
                texture: assets.load("missile-1.png"),
                ..default()
            },
        ));
    }
}

fn update_bullet_graphic(
    targets: Query<(&ShipIntel, &Transform), Without<Traversal>>,
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
        let (target_intel, target_transform) = targets.get(target.ship).unwrap();
        let origin = ships.get(origin.ship).unwrap().translation.xy(); // TODO weapon mount
        let out_mid = Vec2::X * 1000.0;
        let room_center = room_center(target_intel, target.room).extend(0.0);
        let destination =
            (target_transform.rotation * room_center + target_transform.translation).xy();
        let in_mid = destination - 1000.0 * ***incidence;

        bullet.translation = if **traversal < 0.5 {
            origin.lerp(out_mid, **traversal * 2.0)
        } else {
            in_mid.lerp(destination, **traversal * 2.0 - 1.0)
        }
        .extend(Z_BULLETS);
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
    Autofire,
    AllDoors { open: bool },
    SetStations,
    GoStations,
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
    mut targeting: EventWriter<SetProjectileWeaponTarget>,
    mut power: EventWriter<AdjustPower>,
    mut weapon_power: EventWriter<WeaponPower>,
    mut set_autofire: EventWriter<SetAutofire>,
    mut set_doors_open: EventWriter<SetDoorsOpen>,
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
                    targeting.send(SetProjectileWeaponTarget {
                        weapon_index,
                        target: None,
                    });
                    commands.insert_resource(TargetingWeapon(weapon_index));
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
            Controls::SetStations => todo!(),
            Controls::GoStations => todo!(),
        }
    }
}

fn draw_targets(
    windows: Query<&Window, With<PrimaryWindow>>,
    self_intel: Query<&SelfIntel>,
    targets: Query<(&ShipIntel, &Transform)>,
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
            gizmos.circle(world_cursor.extend(Z_BULLETS), Direction3d::Z, size, color);
        }
    }

    for (i, target) in self_intel.weapon_targets.iter().enumerate() {
        if let Some(target) = target {
            let (target_intel, target_transform) = targets.get(target.ship).unwrap();
            let room_location = room_center(target_intel, target.room).extend(2.0);
            let pos = target_transform.rotation * room_location + target_transform.translation;
            let (size, color) = size_color(i);
            gizmos.circle(pos, Direction3d::Z, size, color);
        }
    }
}

fn room_center(intel: &ShipIntel, room: usize) -> Vec2 {
    SHIPS[intel.basic.ship_type].room_center(room)
}
