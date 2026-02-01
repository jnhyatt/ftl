use bevy::{ecs::entity::MapEntities, prelude::*};
use serde::{Deserialize, Serialize};

#[derive(Component, Serialize, Deserialize, Deref, Debug, Clone, Copy, PartialEq, Eq)]
pub struct WeaponDamage(pub usize);

/// Represents the progress of this projectile or beam from being fired (0.0) to reaching its target
/// (1.0).
#[derive(Component, Serialize, Deserialize, Default, Deref, DerefMut, Debug, Clone, Copy)]
pub struct Progress(pub f32);

/// Indicates this projectile hasn't reached the point where dodge is determined yet. Once it
/// crosses the threshold, the dodge test is performed. If dodged, `WeaponDamage` and `ShieldPierce`
/// are removed.
#[derive(Component, Serialize, Deserialize, Default, Clone, Copy)]
pub struct NeedsDodgeTest;

/// This projectile is aiming for a specific room on a ship.
#[derive(Component, MapEntities, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoomTarget {
    /// The ship this projectile should hit if not dodged. We point to the
    /// ship's intel package here because ships are not replicated to clients,
    /// so they would be unable to target anything otherwise.
    #[entities]
    pub ship: Entity,
    pub room: usize,
}

/// This beam will hit a specific ship starting from a specific position and going in a specific
/// direction.
#[derive(Component, MapEntities, Serialize, Deserialize, Debug, Clone, Copy)]
pub struct BeamTarget {
    #[entities]
    pub ship: Entity,
    pub start: Vec2,
    pub dir: Dir2,
}

/// Indicates which ship and weapon this projectile or beam was fired from. This is useful for
/// animating projectiles from the correct position on the ship. Also good if we want to track kills
/// in a game with more than two players (not currently supported).
#[derive(Component, Serialize, Deserialize, Debug, Clone, Copy)]
pub struct FiredFrom {
    #[entities]
    pub ship: Entity,
    pub weapon_index: usize,
}
