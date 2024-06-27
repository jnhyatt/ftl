use super::*;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Event, Serialize, Deserialize, Debug, Clone, Copy)]
pub struct AdjustPower {
    /// Whether to request power from or return power to the reactor.
    pub dir: PowerDir,
    /// Which system power is being adjusted for. The server handles adjusting
    /// power in useful increments -- for example, a single `AdjustPower` event
    /// targeting shields will increase power to shields by two.
    pub system: SystemId,
}

impl AdjustPower {
    pub fn request(system: SystemId) -> Self {
        Self {
            dir: PowerDir::Request,
            system,
        }
    }

    pub fn remove(system: SystemId) -> Self {
        Self {
            dir: PowerDir::Remove,
            system,
        }
    }
}

#[derive(Event, Serialize, Deserialize, Debug, Clone, Copy)]
pub struct WeaponPower {
    /// Whether to request power from or return power to the reactor.
    pub dir: PowerDir,
    /// Which weapon should be powered or depowered.
    pub index: usize,
}

#[derive(Event, Serialize, Deserialize, Debug, Clone, Copy)]
pub struct SetProjectileWeaponTarget {
    pub weapon_index: usize,
    pub target: Option<ProjectileTarget>,
}

impl MapEntities for SetProjectileWeaponTarget {
    fn map_entities<M: EntityMapper>(&mut self, entity_mapper: &mut M) {
        if let Some(target) = &mut self.target {
            target.map_entities(entity_mapper);
        }
    }
}

#[derive(Event, Serialize, Deserialize, Debug, Clone, Copy)]
pub struct MoveWeapon {
    pub weapon_index: usize,
    pub target_index: usize,
}

#[derive(Event, Serialize, Deserialize, Debug, Clone, Copy)]
pub struct SetCrewGoal {
    pub crew: usize,
    pub target_room: usize,
}

#[derive(Event, Serialize, Deserialize, Debug, Clone, Copy)]
pub struct SetAutofire(pub bool);
