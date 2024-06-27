use bevy::{ecs::entity::MapEntities, prelude::*};
use serde::{Deserialize, Serialize};

use crate::{Ship, ShipSystem, SystemId, Weapon};

#[derive(Component, Serialize, Deserialize, Debug, Clone)]
pub struct BasicIntel {
    pub max_hull: usize,
    pub hull: usize,
    pub rooms: Vec<Option<SystemId>>,
    pub shields: Option<ShieldIntel>,
    pub weapons: Option<Vec<WeaponIntel>>,
}

impl BasicIntel {
    pub fn new(ship: &Ship) -> Self {
        Self {
            max_hull: ship.max_hull,
            hull: ship.max_hull - ship.damage,
            rooms: (0..ship.rooms.len())
                .map(|room| ship.systems.system_in_room(room))
                .collect(),
            shields: ship.systems.shields().map(|shields| ShieldIntel {
                max_layers: shields.max_layers(),
                layers: shields.layers,
                charge: shields.charge,
                damage: SystemDamageIntel::from_system(shields),
            }),
            weapons: ship.systems.weapons().map(|weapons| {
                weapons
                    .weapons()
                    .iter()
                    .map(|x| WeaponIntel {
                        weapon: x.weapon.clone(),
                        powered: x.is_powered(),
                        damage: SystemDamageIntel::from_system(weapons),
                    })
                    .collect()
            }),
        }
    }
}

#[derive(Component, Serialize, Deserialize)]
pub struct ShipIntel(pub Entity);

impl MapEntities for ShipIntel {
    fn map_entities<M: EntityMapper>(&mut self, entity_mapper: &mut M) {
        self.0 = entity_mapper.map_entity(self.0);
    }
}

#[derive(Component, Serialize, Deserialize, Debug)]
pub struct IntelPackage {
    pub basic: Entity,
    // pub interior: Entity,
    // pub weapon_charge: Entity,
}

impl MapEntities for IntelPackage {
    fn map_entities<M: EntityMapper>(&mut self, entity_mapper: &mut M) {
        self.basic = entity_mapper.map_entity(self.basic);
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub enum SystemDamageIntel {
    Undamaged,
    Damaged,
    Destroyed,
}

impl SystemDamageIntel {
    fn from_system(system: &dyn ShipSystem) -> Self {
        if system.damage() == system.upgrade_level() {
            Self::Destroyed
        } else if system.damage() == 0 {
            Self::Undamaged
        } else {
            Self::Damaged
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct ShieldIntel {
    pub max_layers: usize,
    pub layers: usize,
    pub charge: f32,
    pub damage: SystemDamageIntel,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WeaponIntel {
    pub weapon: Weapon,
    pub powered: bool,
    pub damage: SystemDamageIntel,
}
