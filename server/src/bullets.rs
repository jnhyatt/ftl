use std::{collections::BTreeMap, time::Duration};

use bevy::{prelude::*, utils::FloatOrd};
use bevy_replicon::core::Replicated;
use common::{
    bullets::{BeamTarget, FiredFrom, NeedsDodgeTest, Progress, RoomTarget, WeaponDamage},
    compute_dodge_chance,
    nav::Cell,
    ship::SHIPS,
    util::{intersect, Aabb},
    weapon::{BeamWeaponId, ProjectileWeaponId},
};
use rand::{thread_rng, Rng};

use crate::{ship::ShipState, ship_system::ShipSystem};

pub fn bullet_traversal(mut projectiles: Query<(&TraversalSpeed, &mut Progress)>) {
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
    projectiles: Query<(Entity, &Progress, &RoomTarget), With<NeedsDodgeTest>>,
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
    projectiles: Query<(Entity, &Progress, &ShieldPierce, &RoomTarget)>,
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
    projectiles: Query<(Entity, &Progress, &RoomTarget, &WeaponDamage)>,
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
            let crew_room = SHIPS[ship.ship_type].cell_room(crew_cell);
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
pub fn projectile_timeout(
    projectiles: Query<(Entity, &Progress, Has<RoomTarget>)>,
    mut commands: Commands,
) {
    for (projectile, &Progress(progress), is_projectile) in &projectiles {
        let max_progress = if is_projectile { 1.5 } else { 1.0 };
        if progress >= max_progress {
            commands.entity(projectile).despawn();
        }
    }
}

pub fn beam_damage(
    mut beams: Query<(&Progress, &BeamTarget, &WeaponDamage, &mut BeamHits)>,
    mut ships: Query<&mut ShipState>,
) {
    for (&progress, target, &damage, mut hits) in &mut beams {
        let Some(next_t) = hits.first_key_value().map(|(&FloatOrd(t), _)| t) else {
            continue;
        };
        let (next_cell, next_room) = if *progress >= next_t {
            hits.pop_first().unwrap().1
        } else {
            continue;
        };
        let mut target = ships.get_mut(target.ship).unwrap();
        let target = target.as_mut();
        let target_ship = &SHIPS[target.ship_type];
        let shield_layers = target.systems.shields.as_mut().map_or(0, |x| x.layers);
        let damage = damage.saturating_sub(shield_layers);

        for crew in &mut target.crew {
            let crew_cell = crew.nav_status.current_cell();
            let crew_room = target_ship.cell_room(crew_cell);
            if crew_room == target_ship.cell_room(next_cell) {
                crew.health -= 15.0 * damage as f32;
            }
        }
        target.crew.retain(|crew| crew.health > 0.0);
        if let Some(next_room) = next_room {
            target.damage = (target.damage + damage).min(target.max_hull);
            if let Some(system) = SHIPS[target.ship_type].room_systems[next_room] {
                if let Some(system) = target.systems.system_mut(system) {
                    system.damage_system(damage, &mut target.reactor);
                }
            }
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
    pub traversal_progress: Progress,
    pub needs_dodge_test: NeedsDodgeTest,
    pub shield_pierce: ShieldPierce,
}

#[derive(Bundle)]
pub struct BeamBundle {
    pub replicated: Replicated,
    pub damage: WeaponDamage,
    pub target: BeamTarget,
    pub hits: BeamHits,
    pub fired_from: FiredFrom,
    pub traversal_speed: TraversalSpeed,
    pub traversal_progress: Progress,
}

#[derive(Component, Deref, Debug, Clone, Copy, PartialEq)]
pub struct TraversalSpeed(pub f32);

#[derive(Component, Deref, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShieldPierce(pub usize);

#[derive(Component, Debug, Deref, DerefMut)]
pub struct BeamHits(BTreeMap<FloatOrd, (Cell, Option<usize>)>);

impl BeamHits {
    pub fn compute(ship_type: usize, beam_len: f32, target: &BeamTarget) -> Self {
        let ship = &SHIPS[ship_type];
        let dir = *target.dir * beam_len;
        // find an intersection `t` for each cell, sort them, map each one to a room and then filter duplicate rooms
        let beam_impact_time = |aabb: Aabb| {
            // Transform aabb into beam space, meaning scale and translate the aabb such that the
            // beam moves from `(0, 0)` to `(1, 1)`.
            let aabb = (aabb - target.start).scale_about_origin(1.0 / dir);
            intersect(0.0..=1.0, aabb.x_range())
                .and_then(|x| intersect(0.0..=1.0, aabb.y_range()).map(|y| (x, y)))
                .and_then(move |(x, y)| intersect(x, y))
                .map(|x| *x.start())
        };
        let mut hits = ship
            .cells()
            .map(|x| (ship.cell_aabb(x), x))
            .filter_map(|(aabb, x)| beam_impact_time(aabb).map(|t| (t, x)))
            .collect::<Vec<_>>();
        hits.sort_by_key(|(t, _)| FloatOrd(*t));
        let mut result = BTreeMap::new();
        for (t, cell) in hits {
            let room = ship.cell_room(cell);
            // if we already hit this room, None, else Some(room)
            let room = result
                .values()
                .all(|&(_, x)| x != Some(room))
                .then_some(room);
            result.insert(FloatOrd(t), (cell, room));
        }
        Self(result)
    }
}

#[derive(Component)]
pub struct DelayedProjectile {
    pub remaining: Duration,
    pub weapon: ProjectileWeaponId,
    pub target: RoomTarget,
    pub fired_from: FiredFrom,
}

#[derive(Component)]
pub struct DelayedBeam {
    pub remaining: Duration,
    pub weapon: BeamWeaponId,
    pub target: BeamTarget,
    pub fired_from: FiredFrom,
}
