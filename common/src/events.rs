use bevy::{ecs::entity::MapEntities, prelude::*};
use serde::{Deserialize, Serialize};

use crate::{
    bullets::{BeamTarget, RoomTarget},
    ship::SystemId,
};

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
    pub weapon_index: usize,
}

#[derive(Event, Serialize, Deserialize, Debug, Clone, Copy)]
pub struct SetProjectileWeaponTarget {
    pub weapon_index: usize,
    pub target: Option<RoomTarget>,
}

impl MapEntities for SetProjectileWeaponTarget {
    fn map_entities<M: EntityMapper>(&mut self, entity_mapper: &mut M) {
        if let Some(target) = &mut self.target {
            target.map_entities(entity_mapper);
        }
    }
}

#[derive(Event, Serialize, Deserialize, Debug, Clone, Copy)]
pub struct SetBeamWeaponTarget {
    pub weapon_index: usize,
    pub target: Option<BeamTarget>,
}

impl MapEntities for SetBeamWeaponTarget {
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
    pub room: usize,
}

#[derive(Event, Serialize, Deserialize, Debug, Clone, Copy)]
pub struct SetAutofire(pub bool);

#[derive(Event, Serialize, Deserialize, Debug, Clone, Copy)]
pub enum SetDoorsOpen {
    Single { door: usize, open: bool },
    All { open: bool },
}

#[derive(Event, Serialize, Deserialize, Debug, Clone, Copy)]
pub enum CrewStations {
    Save,
    Return,
}

#[derive(Reflect, Serialize, Deserialize, Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum PowerDir {
    Request,
    Remove,
}
