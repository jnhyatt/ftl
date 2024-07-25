use std::f32::consts::TAU;

use bevy::{prelude::*, window::PrimaryWindow};
use bevy_mod_picking::prelude::*;
use common::{
    bullets::{BeamTarget, FiredFrom, Progress, RoomTarget},
    intel::{InteriorIntel, SelfIntel, ShipIntel},
    nav::{Cell, CrewNavStatus, LineSection, NavLocation, SquareSection},
    ship::{Dead, Door, DoorDir, SystemId, SHIPS},
    util::inverse_lerp,
    weapon::{WeaponId, WeaponTarget},
};
use rand::{thread_rng, Rng};
use strum::IntoEnumIterator;

use crate::{
    egui_panels::size_color,
    interaction::{handle_cell_click, toggle_door, TargetingWeapon},
    select::Selectable,
};

const Z_BG: f32 = 0.0;
const Z_SHIP: f32 = Z_BG + 1.0;
const Z_BULLETS: f32 = Z_SHIP + Z_SHIELDS + 1.0;

const Z_CELL: f32 = 1.0;
const Z_ICONS: f32 = Z_CELL + Z_WALLS;
const Z_CREW: f32 = Z_ICONS + 1.0;
const Z_SHIELDS: f32 = Z_CREW + 1.0;

const Z_AIR: f32 = 1.0;
const Z_VACUUM: f32 = Z_AIR + 1.0;
const Z_NO_INTEL: f32 = Z_VACUUM + 1.0;
const Z_WALLS: f32 = Z_NO_INTEL + 1.0;

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
pub struct DoorGraphic(pub usize);

#[derive(Component)]
pub struct CrewGraphic(pub usize);

pub fn sync_crew_count(
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

pub fn sync_crew_positions(
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

fn door_sprite(ship_type: usize, index: usize) -> SpriteBundle {
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
    let transform = Transform::from_translation(door_pos).with_rotation(Quat::from_mat3(&Mat3 {
        x_axis: normal.extend(0.0),
        y_axis: normal.perp().extend(0.0),
        z_axis: Vec3::Z,
    }));
    SpriteBundle {
        transform,
        ..default()
    }
}

pub fn add_ship_graphic(
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
        let is_me = ship == my_ship;
        let transform = if is_me {
            println!("{ship:?} is me!");
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
                            SHIPS[intel.basic.ship_type]
                                .room_center(room)
                                .extend(Z_ICONS),
                        )
                        .with_rotation(transform.rotation.inverse()),
                        texture: assets.load(sprite),
                        ..default()
                    },
                )
            })
        };

        for x in SystemId::iter().filter_map(icon) {
            let icon = commands.spawn(x).id();
            commands.entity(ship).add_child(icon);
        }

        commands.entity(ship).with_children(|ship| {
            for i in 0..SHIPS[intel.basic.ship_type].doors.len() {
                let mut e = ship.spawn((DoorGraphic(i), door_sprite(intel.basic.ship_type, i)));
                if is_me {
                    e.insert(On::<Pointer<Click>>::run(toggle_door));
                }
            }
        });

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
                let no_intel = commands
                    .spawn((
                        Pickable::IGNORE,
                        NoIntelGraphic,
                        SpriteBundle {
                            texture: assets.load("no-intel.png"),
                            transform: Transform::from_xyz(0.0, 0.0, Z_NO_INTEL),
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
                commands.entity(cell_graphic).add_child(no_intel);
                commands.entity(cell_graphic).add_child(walls);
            }
        }
    }
}

pub fn update_doors(
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

pub fn update_oxygen(
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

pub fn update_vacuum(
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

pub fn update_no_intel(
    self_intel: Query<&SelfIntel>,
    cells: Query<&Parent>,
    mut no_intel: Query<(&Parent, &mut Visibility), With<NoIntelGraphic>>,
) {
    let Ok(self_intel) = self_intel.get_single() else {
        return;
    };
    for (parent, mut visibility) in &mut no_intel {
        let ship = **cells.get(**parent).unwrap();
        *visibility = if ship == self_intel.ship {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
    }
}

#[derive(Component, Clone, Copy)]
pub struct RoomGraphic(pub usize);

#[derive(Component, Clone, Copy)]
pub struct OxygenGraphic(usize);

#[derive(Component, Clone, Copy)]
pub struct VacuumGraphic(usize);

#[derive(Component, Clone, Copy)]
pub struct NoIntelGraphic;

#[derive(Component, Deref)]
pub struct BulletIncidence(Direction2d);

pub fn spawn_projectile_graphics(
    bullets: Query<Entity, (With<RoomTarget>, Without<Sprite>)>,
    assets: Res<AssetServer>,
    mut commands: Commands,
) {
    for bullet in &bullets {
        commands.entity(bullet).insert((
            Pickable::IGNORE,
            SpriteBundle {
                texture: assets.load("missile-1.png"),
                ..default()
            },
        ));
    }
}

pub fn set_bullet_incidence(
    bullets: Query<Entity, (With<Progress>, Without<BulletIncidence>)>,
    mut commands: Commands,
) {
    for bullet in &bullets {
        let direction =
            Direction2d::new_unchecked(Vec2::from_angle(thread_rng().gen_range(0.0..=TAU)));
        commands.entity(bullet).insert(BulletIncidence(direction));
    }
}

pub fn update_bullet_graphic(
    targets: Query<(&ShipIntel, &Transform), Without<Progress>>,
    ships: Query<&Transform, Without<Progress>>,
    mut bullets: Query<(
        &Progress,
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
        let room_center = {
            let room = target.room;
            SHIPS[target_intel.basic.ship_type].room_center(room)
        }
        .extend(0.0);
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

pub fn draw_beams(
    ships: Query<(&ShipIntel, &GlobalTransform)>,
    beams: Query<(&FiredFrom, &Progress, &BeamTarget, &BulletIncidence)>,
    mut gizmos: Gizmos,
) {
    for (origin, &progress, target, incidence) in &beams {
        let (intel, firing_ship) = ships.get(origin.ship).unwrap();
        let Some(weapons) = &intel.basic.weapons else {
            continue;
        };
        let WeaponId::Beam(weapon) = weapons.weapons[origin.weapon_index].weapon else {
            continue;
        };
        let beam_length = weapon.length;
        let (target_intel, target_ship) = ships.get(target.ship).unwrap();

        let weapon_mount_pos = Vec2::ZERO.extend(Z_BULLETS);
        let beam_start = firing_ship.transform_point(weapon_mount_pos);
        let out_mid = firing_ship.transform_point(weapon_mount_pos + Vec3::X * 1000.0);
        let hit_point = target.start + (*target.dir * beam_length * *progress);
        let in_mid = hit_point + ***incidence * 1000.0;
        let target_shields = target_intel.basic.shields.map_or(0, |x| x.layers);
        let hull_damage = weapon.common.damage.saturating_sub(target_shields);
        let hit_point = if hull_damage == 0 {
            // find the intersection of the line (in_mid, hit_point) with a circle at 150
            let ab = hit_point - in_mid;
            let a_t = ab * ab.dot(in_mid) / ab.length_squared();
            let b_t = ab + a_t;

            let d_sqr = (in_mid - a_t).length_squared();
            let a_t = a_t.dot(ab.normalize());
            let b_t = b_t.dot(ab.normalize());

            let target = 150.0;
            let t = (target * target - d_sqr).sqrt();
            let lerp_low = inverse_lerp(a_t, b_t, -t);
            in_mid.lerp(hit_point, lerp_low)
        } else {
            hit_point
        };
        let in_mid = target_ship.transform_point(in_mid.extend(Z_BULLETS));
        let beam_end = target_ship.transform_point(hit_point.extend(Z_BULLETS));

        gizmos.line(beam_start, out_mid, Color::RED);
        gizmos.line(in_mid, beam_end, Color::RED);
    }
}

pub fn draw_targets(
    windows: Query<&Window, With<PrimaryWindow>>,
    self_intel: Query<&SelfIntel>,
    ships: Query<&ShipIntel>,
    targets: Query<(&ShipIntel, &Transform)>,
    targeting_weapon: Option<Res<TargetingWeapon>>,
    mut gizmos: Gizmos,
) {
    let Ok(self_intel) = self_intel.get_single() else {
        return;
    };
    let Ok(ship) = ships.get(self_intel.ship) else {
        return;
    };
    let Some(weapons) = &ship.basic.weapons else {
        return;
    };

    if let Some(cursor) = windows.get_single().ok().and_then(|x| x.cursor_position()) {
        let world_cursor = cursor * Vec2::new(1.0, -1.0) + Vec2::new(-640.0, 360.0);
        match targeting_weapon.as_ref().map(|x| x.as_ref()) {
            Some(&TargetingWeapon::PickStart { weapon_index }) => {
                let (size, color) = size_color(weapon_index);
                gizmos.circle(world_cursor.extend(Z_BULLETS), Direction3d::Z, size, color);
            }
            Some(&TargetingWeapon::PickDir {
                weapon_index,
                start,
                ..
            }) => {
                let WeaponId::Beam(weapon) = weapons.weapons[weapon_index].weapon else {
                    return;
                };
                let beam_length = weapon.length;
                let (_, color) = size_color(weapon_index);
                let dir = Direction2d::new(world_cursor - start).unwrap_or(Direction2d::Y);
                let end = start + *dir * beam_length;
                gizmos.line(start.extend(Z_BULLETS), end.extend(Z_BULLETS), color);
            }
            _ => {}
        }
    }

    for (i, target) in self_intel.weapon_targets.iter().enumerate() {
        if let Some(target) = target {
            match target {
                WeaponTarget::Projectile(target) => {
                    let (target_intel, target_transform) = targets.get(target.ship).unwrap();
                    let room_location = {
                        let room = target.room;
                        SHIPS[target_intel.basic.ship_type].room_center(room)
                    }
                    .extend(Z_BULLETS);
                    let pos =
                        target_transform.rotation * room_location + target_transform.translation;
                    let (size, color) = size_color(i);
                    gizmos.circle(pos, Direction3d::Z, size, color);
                }
                WeaponTarget::Beam(target) => {
                    let WeaponId::Beam(weapon) = weapons.weapons[i].weapon else {
                        return;
                    };
                    let beam_length = weapon.length;
                    let (_, target_transform) = targets.get(target.ship).unwrap();
                    let start = target.start.extend(Z_BULLETS);
                    let end = (target.start + *target.dir * beam_length).extend(Z_BULLETS);
                    let start = target_transform.rotation * start + target_transform.translation;
                    let end = target_transform.rotation * end + target_transform.translation;
                    let (_, color) = size_color(i);
                    gizmos.line(start, end, color);
                }
            }
        }
    }
}
