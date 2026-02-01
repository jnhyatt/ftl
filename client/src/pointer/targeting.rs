use bevy::prelude::*;
use common::{
    bullets::{BeamTarget, RoomTarget},
    events::SetProjectileWeaponTarget,
    intel::{SelfIntel, ShipIntel},
    weapon::WeaponId,
};

use crate::{graphics::RoomGraphic, targeting::target_beam_weapon};

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
pub struct DisableWhenTargeting;

#[derive(Component)]
pub struct EnableWhenTargeting;

/// When targeting a beam weapon, the start point must be in a room of the enemy ship, so that
/// observer sits on the cell entities. For choosing the direction, the user can click anywhere, so
/// this observer sits on the screen quad and is only active when choosing a beam direction.
pub fn aim_beam(
    event: On<Pointer<Press>>,
    targeting_weapon: Res<TargetingWeapon>,
    ships: Query<&GlobalTransform>,
    mut commands: Commands,
) -> Result {
    let PointerButton::Primary = event.button else {
        return Ok(());
    };
    let world_cursor = event
        .hit
        .position
        .ok_or("sprite backend must give us a position")?
        .xy();
    let &TargetingWeapon::PickDir { ship, start, .. } = &*targeting_weapon else {
        return Ok(());
    };
    let ship_transform = ships.get(ship)?;
    let world_to_ship = ship_transform.affine().inverse();
    let start = world_to_ship.transform_point(start.extend(0.0)).xy();
    let end = world_to_ship.transform_point(world_cursor.extend(0.0)).xy();
    let dir = Dir2::new(end - start).unwrap_or(Dir2::Y);
    commands.queue(target_beam_weapon(Some(BeamTarget { ship, start, dir })));
    Ok(())
}

/// Observer that targets the currently selected weapon at this cell's room.
pub fn target_weapon(
    event: On<Pointer<Press>>,
    weapon: Option<Res<TargetingWeapon>>,
    self_intel: Single<&SelfIntel>,
    ships: Query<&ShipIntel>,
    cells: Query<(&RoomGraphic, &ChildOf)>,
    mut projectile_targeting: MessageWriter<SetProjectileWeaponTarget>,
    mut commands: Commands,
) -> Result {
    let PointerButton::Primary = event.button else {
        return Ok(());
    };
    let Some(&TargetingWeapon::PickStart { weapon_index }) = weapon.as_ref().map(|x| x.as_ref())
    else {
        return Ok(());
    };
    let (&RoomGraphic(room), &ChildOf(ship)) = cells.get(event.entity)?;

    let client_ship = self_intel.ship;
    let client_intel = ships.get(client_ship).unwrap();
    let weapon = &client_intel.basic.weapons.as_ref().unwrap().weapons[weapon_index].weapon;
    if ship == client_ship {
        // If we're targeting self, make sure that's ok
        let can_target_self = if let WeaponId::Projectile(weapon) = weapon {
            weapon.can_target_self
        } else {
            false
        };
        if !can_target_self {
            // If we can't target self, discard the pointer press.
            return Ok(());
        }
    }
    match weapon {
        WeaponId::Projectile(_) => {
            projectile_targeting.write(SetProjectileWeaponTarget {
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
    Ok(())
}

pub fn cancel_targeting(event: On<Pointer<Press>>, mut commands: Commands) {
    if event.button == PointerButton::Secondary {
        commands.remove_resource::<TargetingWeapon>();
    }
}
