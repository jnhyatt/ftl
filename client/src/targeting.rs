use bevy::prelude::*;
use common::{
    bullets::BeamTarget,
    events::{SetBeamWeaponTarget, SetProjectileWeaponTarget},
    intel::{SelfIntel, ShipIntel},
    weapon::WeaponId,
};

use crate::pointer::targeting::TargetingWeapon;

pub fn start_targeting(weapon_index: usize) -> impl Command {
    move |world: &mut World| {
        let Ok(ship) = world.query::<&SelfIntel>().single(world).map(|x| x.ship) else {
            return;
        };
        let Ok(ship) = world.query::<&ShipIntel>().get(world, ship) else {
            return;
        };
        let Some(weapons) = &ship.basic.weapons else {
            return;
        };
        // Clear any existing targeting even though we don't have a replacement yet so the user can
        // rapidly detarget weapons.
        match weapons.weapons[weapon_index].weapon {
            WeaponId::Projectile(_) => {
                world.write_message(SetProjectileWeaponTarget {
                    weapon_index,
                    target: None,
                });
            }
            WeaponId::Beam(_) => {
                world.write_message(SetBeamWeaponTarget {
                    weapon_index,
                    target: None,
                });
            }
        }
        world.insert_resource(TargetingWeapon::PickStart { weapon_index });
    }
}

pub fn target_beam_weapon(target: Option<BeamTarget>) -> impl Command {
    move |world: &mut World| {
        let Some(TargetingWeapon::PickDir { weapon_index, .. }) =
            world.remove_resource::<TargetingWeapon>()
        else {
            return;
        };
        world.write_message(SetBeamWeaponTarget {
            weapon_index,
            target,
        });
    }
}
