use bevy::{ecs::entity::MapEntities, prelude::*};
use bevy_replicon::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Component, Serialize, Deserialize, Deref, Debug, Clone, Copy, PartialEq, Eq)]
pub struct WeaponDamage(pub usize);

#[derive(Component, Deref, Debug, Clone, Copy, PartialEq)]
pub struct TraversalSpeed(pub f32);

#[derive(
    Component, Serialize, Deserialize, Default, Deref, DerefMut, Debug, Clone, Copy, PartialEq,
)]
pub struct Traversal(pub f32);

#[derive(Component, Deref, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShieldPierce(pub usize);

#[derive(Component, Serialize, Deserialize, Default, Clone, Copy)]
pub struct NeedsDodgeTest;

#[derive(Component, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProjectileTarget {
    /// The ship this projectile should hit if not dodged. We point to the
    /// ship's intel package here because ships are not replicated to clients,
    /// so they would be unable to target anything otherwise.
    pub ship_intel: Entity,
    pub room: usize,
}

impl MapEntities for ProjectileTarget {
    fn map_entities<M: EntityMapper>(&mut self, entity_mapper: &mut M) {
        self.ship_intel = entity_mapper.map_entity(self.ship_intel);
    }
}

#[derive(Bundle)]
pub struct ProjectileBundle {
    pub replicated: Replicated,
    pub damage: WeaponDamage,
    pub target: ProjectileTarget,
    pub traversal_speed: TraversalSpeed,
    pub traversal_progress: Traversal,
    pub needs_dodge_test: NeedsDodgeTest,
    pub shield_pierce: ShieldPierce,
}
