use bevy::{ecs::entity::MapEntities, prelude::*};
use serde::{Deserialize, Serialize};

#[derive(Component, Serialize, Deserialize, Deref, Debug, Clone, Copy, PartialEq, Eq)]
pub struct WeaponDamage(pub usize);

#[derive(Component, Serialize, Deserialize, Default, Deref, DerefMut, Debug, Clone, Copy)]
pub struct Progress(pub f32);

#[derive(Component, Serialize, Deserialize, Default, Clone, Copy)]
pub struct NeedsDodgeTest;

#[derive(Component, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoomTarget {
    /// The ship this projectile should hit if not dodged. We point to the
    /// ship's intel package here because ships are not replicated to clients,
    /// so they would be unable to target anything otherwise.
    pub ship: Entity,
    pub room: usize,
}

impl MapEntities for RoomTarget {
    fn map_entities<M: EntityMapper>(&mut self, entity_mapper: &mut M) {
        self.ship = entity_mapper.map_entity(self.ship);
    }
}

#[derive(Component, Serialize, Deserialize, Debug, Clone, Copy)]
pub struct BeamTarget {
    pub ship: Entity,
    pub start: Vec2,
    pub dir: Dir2,
}

impl MapEntities for BeamTarget {
    fn map_entities<M: EntityMapper>(&mut self, entity_mapper: &mut M) {
        self.ship = entity_mapper.map_entity(self.ship);
    }
}

#[derive(Component, Serialize, Deserialize, Debug, Clone, Copy)]
pub struct FiredFrom {
    pub ship: Entity,
    pub weapon_index: usize,
}

impl MapEntities for FiredFrom {
    fn map_entities<M: EntityMapper>(&mut self, entity_mapper: &mut M) {
        self.ship = entity_mapper.map_entity(self.ship);
    }
}
