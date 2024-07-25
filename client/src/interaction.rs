use bevy::{ecs::system::Command, prelude::*};
use bevy_mod_picking::prelude::*;
use common::{
    bullets::{BeamTarget, RoomTarget},
    events::{SetBeamWeaponTarget, SetCrewGoal, SetDoorsOpen, SetProjectileWeaponTarget},
    intel::{SelfIntel, ShipIntel},
    ship::Dead,
    util::{disable, enable},
    weapon::WeaponId,
};

use crate::{
    graphics::{CrewGraphic, DoorGraphic, RoomGraphic},
    select::{SelectEvent, Selected},
};

pub fn start_targeting(weapon_index: usize) -> impl Command {
    move |world: &mut World| {
        let Ok(ship) = world
            .query::<&SelfIntel>()
            .get_single(world)
            .map(|x| x.ship)
        else {
            return;
        };
        let Ok(ship) = world.query::<&ShipIntel>().get(world, ship) else {
            return;
        };
        let Some(weapons) = &ship.basic.weapons else {
            return;
        };
        match weapons.weapons[weapon_index].weapon {
            WeaponId::Projectile(_) => {
                world.send_event(SetProjectileWeaponTarget {
                    weapon_index,
                    target: None,
                });
            }
            WeaponId::Beam(_) => {
                world.send_event(SetBeamWeaponTarget {
                    weapon_index,
                    target: None,
                });
            }
        }
        world.insert_resource(TargetingWeapon::PickStart { weapon_index });
        let pick_root = world
            .query_filtered::<Entity, With<PickRoot>>()
            .single(world);
        disable::<On<Pointer<Down>>>(pick_root, world);
    }
}

#[derive(Resource, Debug)]
pub enum TargetingWeapon {
    PickStart {
        weapon_index: usize,
    },
    PickDir {
        weapon_index: usize,
        ship: Entity,
        start: Vec2,
    },
}

#[derive(Component)]
pub struct PickRoot;

pub fn left_click_background(
    event: Listener<Pointer<Down>>,
    targeting_weapon: Option<Res<TargetingWeapon>>,
    ships: Query<&GlobalTransform>,
    cameras: Query<(&Camera, &GlobalTransform)>,
    mut beam_targeting: EventWriter<SetBeamWeaponTarget>,
    mut select_events: EventWriter<SelectEvent>,
    mut commands: Commands,
) {
    if let PointerButton::Primary = event.button {
        let (camera, camera_transform) = cameras.single();
        let Some(world_cursor) =
            camera.viewport_to_world_2d(camera_transform, event.pointer_location.position)
        else {
            return;
        };
        if let Some(targeting_weapon) = targeting_weapon.as_ref().map(|x| x.as_ref()) {
            let &TargetingWeapon::PickDir {
                weapon_index,
                ship,
                start,
            } = targeting_weapon
            else {
                return;
            };
            commands.remove_resource::<TargetingWeapon>();
            let Ok(ship_transform) = ships.get(ship) else {
                return;
            };
            let world_to_ship = ship_transform.affine().inverse();
            let start = world_to_ship.transform_point(start.extend(0.0)).xy();
            let end = world_to_ship.transform_point(world_cursor.extend(0.0)).xy();
            let dir = Direction2d::new(end - start).unwrap_or(Direction2d::Y);

            beam_targeting.send(SetBeamWeaponTarget {
                weapon_index,
                target: Some(BeamTarget { ship, start, dir }),
            });
        } else {
            select_events.send(SelectEvent::GrowTo(world_cursor));
        }
    }
}

pub fn handle_cell_click(
    event: Listener<Pointer<Down>>,
    weapon: Option<Res<TargetingWeapon>>,
    self_intel: Query<&SelfIntel>,
    ships: Query<&ShipIntel>,
    cells: Query<(&RoomGraphic, &Parent)>,
    selected_crew: Query<&CrewGraphic, With<Selected>>,
    pick_root: Query<Entity, With<PickRoot>>,
    mut projectile_targeting: EventWriter<SetProjectileWeaponTarget>,
    mut set_crew_goal: EventWriter<SetCrewGoal>,
    mut commands: Commands,
) {
    let (&RoomGraphic(room), parent) = cells.get(event.target).unwrap();
    match event.button {
        PointerButton::Primary => {
            // Target selected weapon at this cell's room
            let Some(&TargetingWeapon::PickStart { weapon_index }) =
                weapon.as_ref().map(|x| x.as_ref())
            else {
                return;
            };
            let ship = **parent;
            let client_ship = self_intel.single().ship;
            let client_intel = ships.get(client_ship).unwrap();
            let weapon = &client_intel.basic.weapons.as_ref().unwrap().weapons[weapon_index].weapon;
            if ship == client_ship {
                // If we're targeting self, make sure that's ok
                let can_target_self = if let WeaponId::Projectile(weapon) = weapon {
                    weapon.can_target_self
                } else {
                    false
                };
                if can_target_self {
                    return;
                }
            }
            commands
                .entity(pick_root.single())
                .add(enable::<On<Pointer<Down>>>);
            match weapon {
                WeaponId::Projectile(_) => {
                    projectile_targeting.send(SetProjectileWeaponTarget {
                        target: Some(RoomTarget { ship, room }),
                        weapon_index,
                    });
                    commands.remove_resource::<TargetingWeapon>();
                }
                WeaponId::Beam(_) => {
                    commands.insert_resource(TargetingWeapon::PickDir {
                        weapon_index,
                        ship,
                        start: event.hit.position.unwrap().xy(),
                    });
                }
            }
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

pub fn toggle_door(
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
