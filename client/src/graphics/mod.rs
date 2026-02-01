mod crew;

use crate::{
    egui_panels::size_color,
    graphics::crew::{sync_crew_count, sync_crew_positions},
    pointer::{
        set_crew_goal,
        targeting::{target_weapon, DisableWhenTargeting, EnableWhenTargeting, TargetingWeapon},
        toggle_door,
    },
};
use bevy::{
    color::palettes,
    math::{CompassOctant, CompassQuadrant},
    prelude::*,
};
use common::{
    bullets::{BeamTarget, FiredFrom, Progress, RoomTarget},
    intel::{InteriorIntel, SelfIntel, ShipIntel},
    nav::Cell,
    ship::{Dead, Door, SystemId, SHIPS},
    util::{inverse_lerp, DisabledObserver},
    weapon::{WeaponId, WeaponTarget},
};
use rand::{thread_rng, Rng};
use std::f32::consts::TAU;
use strum::IntoEnumIterator;

pub fn graphics_plugin(app: &mut App) {
    app.add_systems(
        Update,
        (sync_crew_count, sync_crew_positions)
            .chain()
            .run_if(any_with_component::<SelfIntel>),
    );
}

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

#[derive(Component)]
pub struct DoorGraphic(pub usize);

#[derive(Component)]
pub struct CrewGraphic(pub usize);

fn walls_tex(assets: &AssetServer, x: CompassOctant) -> Handle<Image> {
    assets.load(match x {
        CompassOctant::NorthEast => "walls-corner.png",
        CompassOctant::NorthWest => "walls-corner.png",
        CompassOctant::SouthWest => "walls-corner.png",
        CompassOctant::SouthEast => "walls-corner.png",
        CompassOctant::North => "walls-edge.png",
        CompassOctant::West => "walls-edge.png",
        CompassOctant::South => "walls-edge.png",
        CompassOctant::East => "walls-edge.png",
    })
}

fn door_transform(ship_type: usize, index: usize) -> Transform {
    let ship = &SHIPS[ship_type];
    let cells = ship.cell_positions;
    let door_pos = match ship.doors[index] {
        Door::Interior(a, b) => (cells[a.0] + cells[b.0]) / 2.0,
        Door::Exterior(cell, dir) => cells[cell.0] + Dir2::from(dir) * 17.5,
    };
    let normal = match ship.doors[index] {
        Door::Interior(a, b) => Dir2::new(cells[b.0] - cells[a.0]).unwrap(),
        Door::Exterior(_, dir) => Dir2::from(dir),
    };
    Transform::from_translation(door_pos.extend(Z_CREW)).with_rotation(Quat::from_mat3(&Mat3 {
        x_axis: normal.extend(0.0),
        y_axis: normal.perp().extend(0.0),
        z_axis: Vec3::Z,
    }))
}

// This absolute travesty needs to be broken into bits. Also turned into an observer. Or several.
//
// It loops through all ships that don't yet have a sprite and:
// - determines transform based on whether it's the player's ship or not
// - adds the ship sprite
// - adds system icons
// - adds door graphics with observers for toggling doors if it's the player's ship
// - adds cell graphics, including oxygen, vacuum, walls and no-intel overlays
pub fn add_ship_graphic(
    self_intel: Single<&SelfIntel>,
    ships: Query<(Entity, &ShipIntel), Without<Sprite>>,
    assets: Res<AssetServer>,
    mut commands: Commands,
) {
    let my_ship = self_intel.ship;
    for (ship, intel) in &ships {
        let is_me = ship == my_ship;
        let transform = if is_me {
            Transform::from_xyz(-200.0, 0.0, Z_SHIP)
        } else {
            Transform::from_xyz(400.0, 0.0, Z_SHIP).with_rotation(Quat::from_rotation_z(TAU / 4.0))
        };

        commands.entity(ship).insert((
            Sprite {
                image: assets.load("cyclops.png"),
                ..default()
            },
            transform,
        ));

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
                    Name::new(format!("Icon for {system}")),
                    Sprite {
                        image: assets.load(sprite),
                        ..default()
                    },
                    Transform::from_translation(
                        SHIPS[intel.basic.ship_type]
                            .room_center(room)
                            .extend(Z_ICONS),
                    )
                    .with_rotation(transform.rotation.inverse()),
                )
            })
        };

        for x in SystemId::iter().filter_map(icon) {
            let icon = commands.spawn(x).id();
            commands.entity(ship).add_child(icon);
        }

        commands.entity(ship).with_children(|ship| {
            for i in 0..SHIPS[intel.basic.ship_type].doors.len() {
                let mut e = ship.spawn((
                    Name::new(format!("Door {i}")),
                    DoorGraphic(i),
                    Sprite::default(),
                    Pickable::default(),
                    door_transform(intel.basic.ship_type, i),
                ));
                if is_me {
                    e.observe(toggle_door);
                }
            }
        });

        for (room_index, room) in SHIPS[intel.basic.ship_type].rooms.iter().enumerate() {
            let room_center = SHIPS[intel.basic.ship_type].room_center(room_index);
            for &Cell(cell) in room.cells {
                use std::cmp::Ordering::*;
                use CompassQuadrant::*;
                let cells = &SHIPS[intel.basic.ship_type].cell_positions;
                let tex = match (
                    cells[cell].x.total_cmp(&room_center.x),
                    cells[cell].y.total_cmp(&room_center.y),
                ) {
                    (Less, Less) => CompassOctant::SouthWest,
                    (Less, Equal) => CompassOctant::West,
                    (Less, Greater) => CompassOctant::NorthWest,
                    (Equal, Less) => CompassOctant::South,
                    (Equal, Greater) => CompassOctant::North,
                    (Greater, Less) => CompassOctant::SouthEast,
                    (Greater, Equal) => CompassOctant::East,
                    (Greater, Greater) => CompassOctant::NorthEast,
                    (Equal, Equal) => panic!("No center tiles"),
                };
                let wall_rotation = match tex {
                    CompassOctant::NorthEast => Quat::from_rotation_z(TAU * 0.5),
                    CompassOctant::North => Quat::from_rotation_z(TAU * 0.5),
                    CompassOctant::NorthWest => Quat::from_rotation_z(TAU * 0.75),
                    CompassOctant::West => Quat::from_rotation_z(TAU * 0.75),
                    CompassOctant::SouthWest => Quat::from_rotation_z(TAU * 0.0),
                    CompassOctant::South => Quat::from_rotation_z(TAU * 0.0),
                    CompassOctant::SouthEast => Quat::from_rotation_z(TAU * 0.25),
                    CompassOctant::East => Quat::from_rotation_z(TAU * 0.25),
                };
                let wall_caps: &'static [CompassQuadrant] = match tex {
                    CompassOctant::NorthEast => &[East, North],
                    CompassOctant::North => &[East, North, West],
                    CompassOctant::NorthWest => &[North, West],
                    CompassOctant::West => &[North, West, South],
                    CompassOctant::SouthWest => &[West, South],
                    CompassOctant::South => &[West, South, East],
                    CompassOctant::SouthEast => &[South, East],
                    CompassOctant::East => &[South, East, North],
                };
                let cell_graphic = commands
                    .spawn((
                        RoomGraphic(room_index),
                        Name::new(format!("Room {room_index} cell {cell} background")),
                        Sprite {
                            image: assets.load("cell.png"),
                            ..default()
                        },
                        Transform::from_translation(cells[cell].extend(Z_CELL)),
                        Pickable::default(),
                    ))
                    .id();
                commands.spawn((
                    EnableWhenTargeting,
                    DisabledObserver(Observer::new(target_weapon).with_entity(cell_graphic)),
                ));
                commands.spawn((
                    DisableWhenTargeting,
                    Observer::new(set_crew_goal).with_entity(cell_graphic),
                ));
                let oxygen = commands
                    .spawn((
                        Pickable::IGNORE,
                        Name::new(format!("Room {room_index} cell {cell} O2 overlay")),
                        OxygenGraphic(room_index),
                        Sprite {
                            image: assets.load("low-oxygen.png"),
                            ..default()
                        },
                        Transform::from_xyz(0.0, 0.0, Z_AIR),
                    ))
                    .id();
                let vacuum = commands
                    .spawn((
                        Pickable::IGNORE,
                        Name::new(format!("Room {room_index} cell {cell} vacuum overlay")),
                        VacuumGraphic(room_index),
                        Sprite {
                            image: assets.load("vacuum.png"),
                            ..default()
                        },
                        Transform::from_xyz(0.0, 0.0, Z_VACUUM),
                    ))
                    .id();
                let walls = commands
                    .spawn((
                        Pickable::IGNORE,
                        Name::new(format!("Room {room_index} cell {cell} walls")),
                        Sprite {
                            image: walls_tex(assets.as_ref(), tex),
                            ..default()
                        },
                        Transform::from_xyz(0.0, 0.0, Z_WALLS).with_rotation(wall_rotation),
                    ))
                    .id();
                let no_intel = commands
                    .spawn((
                        Pickable::IGNORE,
                        Name::new(format!("Room {room_index} cell {cell} no intel overlay")),
                        NoIntelGraphic,
                        Sprite {
                            image: assets.load("no-intel.png"),
                            ..default()
                        },
                        Transform::from_xyz(0.0, 0.0, Z_NO_INTEL),
                    ))
                    .id();

                let door_positions = SHIPS[intel.basic.ship_type]
                    .doors
                    .iter()
                    .map(|x| match x {
                        Door::Interior(a, b) => (cells[a.0] + cells[b.0]) / 2.0,
                        Door::Exterior(cell, dir) => cells[cell.0] + Dir2::from(*dir) * 17.5,
                    })
                    .collect::<Vec<_>>();
                for &cap in wall_caps {
                    if !door_positions.contains(&(cells[cell] + Dir2::from(cap) * 17.5)) {
                        let rotation = match cap {
                            CompassQuadrant::East => Quat::from_rotation_z(TAU * 0.0),
                            CompassQuadrant::North => Quat::from_rotation_z(TAU * 0.25),
                            CompassQuadrant::West => Quat::from_rotation_z(TAU * 0.5),
                            CompassQuadrant::South => Quat::from_rotation_z(TAU * 0.75),
                        };
                        let cap = commands
                            .spawn((
                                Pickable::IGNORE,
                                Name::new(format!("Room {room_index} cell {cell} wall cap")),
                                Sprite {
                                    image: assets.load("wall-cap.png"),
                                    ..default()
                                },
                                Transform::from_translation(
                                    (Dir2::from(cap) * 17.5).extend(Z_WALLS),
                                )
                                .with_rotation(rotation),
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

pub fn sync_door_sprites(
    ships: Query<&ShipIntel>,
    mut doors: Query<(&DoorGraphic, &ChildOf, &mut Sprite)>,
    assets: Res<AssetServer>,
) -> Result {
    for (&DoorGraphic(door), &ChildOf(parent), mut sprite) in &mut doors {
        let ship = ships.get(parent)?;
        let door = ship.basic.doors[door];
        sprite.image = match door.open {
            _ if door.broken() => assets.load("door-broken.png"),
            false => assets.load("door-closed.png"),
            true => assets.load("door-open.png"),
        };
    }
    Ok(())
}

pub fn sync_oxygen_overlays(
    ships: Query<&ShipIntel, Without<Dead>>,
    interiors: Query<&InteriorIntel>,
    cells: Query<&ChildOf>,
    mut oxygen: Query<(&OxygenGraphic, &ChildOf, &mut Sprite)>,
) -> Result {
    for (&OxygenGraphic(room), &ChildOf(cell), mut sprite) in &mut oxygen {
        let &ChildOf(ship) = cells.get(cell)?;
        let Ok(ship) = ships.get(ship) else {
            continue;
        };
        let Ok(interior) = interiors.get(ship.interior) else {
            continue;
        };
        sprite.color.set_alpha(1.0 - interior.rooms[room].oxygen);
    }
    Ok(())
}

pub fn sync_vacuum_overlays(
    ships: Query<&ShipIntel, Without<Dead>>,
    interiors: Query<&InteriorIntel>,
    cells: Query<&ChildOf>,
    mut oxygen: Query<(&VacuumGraphic, &ChildOf, &mut Visibility)>,
) -> Result {
    for (&VacuumGraphic(room), &ChildOf(cell), mut visibility) in &mut oxygen {
        let &ChildOf(ship) = cells.get(cell)?;
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
    Ok(())
}

pub fn sync_no_intel_overlays(
    cells: Query<&ChildOf>,
    has_interior_intel: Query<Has<InteriorIntel>>,
    mut no_intel: Query<(&ChildOf, &mut Visibility), With<NoIntelGraphic>>,
) -> Result {
    for (&ChildOf(cell), mut visibility) in &mut no_intel {
        let &ChildOf(ship) = cells.get(cell)?;
        *visibility = if has_interior_intel.get(ship)? {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
    }
    Ok(())
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
pub struct BulletIncidence(Dir2);

pub fn spawn_projectile_graphics(
    bullets: Query<Entity, (With<RoomTarget>, Without<Sprite>)>,
    assets: Res<AssetServer>,
    mut commands: Commands,
) {
    for bullet in &bullets {
        commands.entity(bullet).insert((
            Pickable::IGNORE,
            Sprite {
                image: assets.load("missile-1.png"),
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
        let direction = Dir2::new_unchecked(Vec2::from_angle(thread_rng().gen_range(0.0..=TAU)));
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

        gizmos.line(beam_start, out_mid, palettes::basic::RED);
        gizmos.line(in_mid, beam_end, palettes::basic::RED);
    }
}

pub fn draw_targets(
    window: Single<&Window>,
    self_intel: Single<&SelfIntel>,
    ships: Query<&ShipIntel>,
    targets: Query<(&ShipIntel, &Transform)>,
    targeting_weapon: Option<Res<TargetingWeapon>>,
    mut gizmos: Gizmos,
) -> Result {
    let ship = ships.get(self_intel.ship)?;
    let Some(weapons) = &ship.basic.weapons else {
        return Ok(());
    };

    if let Some(cursor) = window.cursor_position() {
        let world_cursor = cursor * Vec2::new(1.0, -1.0) + Vec2::new(-640.0, 360.0);
        match targeting_weapon.as_ref().map(|x| x.as_ref()) {
            Some(&TargetingWeapon::PickStart { weapon_index }) => {
                let (size, color) = size_color(weapon_index);
                gizmos.circle(world_cursor.extend(Z_BULLETS), size, color);
            }
            Some(&TargetingWeapon::PickDir {
                weapon_index,
                start,
                ..
            }) => {
                let WeaponId::Beam(weapon) = weapons.weapons[weapon_index].weapon else {
                    return Ok(());
                };
                let beam_length = weapon.length;
                let (_, color) = size_color(weapon_index);
                let dir = Dir2::new(world_cursor - start).unwrap_or(Dir2::Y);
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
                    gizmos.circle(pos, size, color);
                }
                WeaponTarget::Beam(target) => {
                    let WeaponId::Beam(weapon) = weapons.weapons[i].weapon else {
                        return Ok(());
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
    Ok(())
}
