use std::time::Duration;

use bevy::prelude::*;
use bevy_replicon::core::Replicated;
use common::{
    compute_dodge_chance,
    projectiles::{FiredFrom, NeedsDodgeTest, RoomTarget, Traversal, WeaponDamage},
    ship::SHIPS,
    weapon::WeaponType,
};
use rand::{thread_rng, Rng};

use crate::{ship::ShipState, ship_system::ShipSystem};

pub fn projectile_traversal(mut projectiles: Query<(&TraversalSpeed, &mut Traversal)>) {
    for (&TraversalSpeed(speed), mut progress) in &mut projectiles {
        **progress += speed / 64.0;
    }
}

/// Once a projectile reaches a certain point (say, 80% traversal) we need to
/// check if the ship dodges. At that point, we determine the effective dodge
/// chance of the target and decide whether the projectile hit. If it hits, we
/// remove `NeedsDodgeTest` so this system doesn't pick it up again. If it
/// misses, we simply remove `ShieldPierce` and `Damage` so the projectile
/// doesn't interact with the shields or hull. Dodge chance is equal to 5% per
/// unit power in the target's engines subsystem.
pub fn projectile_test_dodge(
    projectiles: Query<(Entity, &Traversal, &RoomTarget), With<NeedsDodgeTest>>,
    ships: Query<&ShipState>,
    mut commands: Commands,
) {
    for (projectile, &progress, target) in &projectiles {
        if *progress < 0.8 {
            continue;
        }
        let ship = ships.get(target.ship).unwrap();
        let dodge_chance = ship
            .systems
            .engines
            .as_ref()
            .map(|engines| compute_dodge_chance(engines.current_power()))
            .unwrap_or_default();
        let roll = thread_rng().gen_range(0..100);
        if roll < dodge_chance {
            commands
                .entity(projectile)
                .remove::<(WeaponDamage, ShieldPierce)>();
        }
        commands.entity(projectile).remove::<NeedsDodgeTest>();
    }
}

/// Once a projectile reaches the shields (say, 85% traversal) we decide how it
/// interacts. The interaction depends on the weapon's shield pierce. If our
/// shield pierce is higher than the target's shields at this point, we simply
/// remove the projectile's `ShieldPierce` so this system doesn't pick it up
/// again. The projectile will continue through to the ship hull. Otherwise, we
/// need to decrement the target's shield and despawn the projectile.
pub fn projectile_shield_interact(
    projectiles: Query<(Entity, &Traversal, &ShieldPierce, &RoomTarget)>,
    mut ships: Query<&mut ShipState>,
    mut commands: Commands,
) {
    for (projectile, &progress, &shield_pierce, target) in &projectiles {
        if *progress < 0.85 {
            continue;
        }
        let mut ship = ships.get_mut(target.ship).unwrap();
        let Some(shields) = ship.systems.shields.as_mut() else {
            continue;
        };
        if *shield_pierce >= shields.layers {
            commands.entity(projectile).remove::<ShieldPierce>();
        } else {
            shields.layers -= 1;
            commands.entity(projectile).despawn();
        }
    }
}

/// Once a projectile reaches 100% traversal, it impacts the hull. We deal
/// damage to the target hull and system (if the target room houses a system)
/// and despawn the projectile.
pub fn projectile_collide_hull(
    projectiles: Query<(Entity, &Traversal, &RoomTarget, &WeaponDamage)>,
    mut ships: Query<&mut ShipState>,
    mut commands: Commands,
) {
    for (projectile, &progress, target, &damage) in &projectiles {
        if *progress < 1.0 {
            continue;
        }

        let mut ship = ships.get_mut(target.ship).unwrap();
        let ship = ship.as_mut();
        ship.damage = (ship.damage + *damage).min(ship.max_hull);
        commands.entity(projectile).despawn();
        for crew in &mut ship.crew {
            let crew_cell = crew.nav_status.current_cell();
            let crew_room = SHIPS[ship.ship_type]
                .rooms
                .iter()
                .position(|x| x.cells.iter().any(|x| *x == crew_cell))
                .unwrap();
            if crew_room == target.room {
                crew.health -= 15.0 * *damage as f32;
            }
        }
        ship.crew.retain(|crew| crew.health > 0.0);
        if let Some(system) = SHIPS[ship.ship_type].room_systems[target.room] {
            if let Some(system) = ship.systems.system_mut(system) {
                system.damage_system(*damage, &mut ship.reactor);
            }
        }
    }
}

/// Not sure about this one still, but I don't necessarily want to despawn
/// projectiles straight away. Instead, we'll let them continue on and ignore
/// them until they reach 150% traversal and are completely offscreen, then
/// despawn them.
pub fn projectile_timeout(projectiles: Query<(Entity, &Traversal)>, mut commands: Commands) {
    for (projectile, &Traversal(progress)) in &projectiles {
        if progress >= 1.5 {
            commands.entity(projectile).despawn();
        }
    }
}

#[derive(Bundle)]
pub struct ProjectileBundle {
    pub replicated: Replicated,
    pub damage: WeaponDamage,
    pub target: RoomTarget,
    pub fired_from: FiredFrom,
    pub traversal_speed: TraversalSpeed,
    pub traversal_progress: Traversal,
    pub needs_dodge_test: NeedsDodgeTest,
    pub shield_pierce: ShieldPierce,
}

#[derive(Component, Deref, Debug, Clone, Copy, PartialEq)]
pub struct TraversalSpeed(pub f32);

#[derive(Component, Deref, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShieldPierce(pub usize);

#[derive(Component)]
pub struct Delayed {
    pub remaining: Duration,
    pub weapon: WeaponType,
    pub target: RoomTarget,
    pub fired_from: FiredFrom,
}
