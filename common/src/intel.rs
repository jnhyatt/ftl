//! Intel represents what a player knows in-game. This is broken down into intel chunks, and each
//! chunk gets its own entity. This is because at the moment we use entity-level replication
//! visibility. To allow a client to see intel chunks, the server makes them visible to that client.
//! This is also the case for the player's own ship, because if a player doesn't have functioning
//! sensors, they can't see their own interior (except where they have crew).
//!
//! The intel chunks are:
//! - **Basic**: Information for a single ship visible even without functioning sensors. This
//! includes shield state, system locations, weapons and their power states, etc.
//! - **Crew vision**: Ship interior as seen by crew. This is typically limited to rooms occupied by
//! a player's crew (slugs being an exception) and can include data from any ship.
//! - **Interior**: Full ship interior for a single ship.
//! - **Weapon charge**: Exact charge levels for weapons.
//! - **Systems**: Full intel about a ship's systems, including upgrade level, power, damage and
//! ion.
//! - **Crew locations**: Exact locations for all crew in all ships. Only available with a slug
//! crewmember.
//!
//! These chunks are given for enemy ships at the following sensor levels:
//! - **No/disabled sensors**: Basic information for all ships, crew vision for own crew and weapon
//! charge and systems for own ship.
//! - **Level 1 sensors**: Interior intel for own ship.
//! - **Level 2 sensors**: Interior intel for enemy ships.
//! - **Level 3 sensors**: Weapon charge for enemy ships.
//! - **Level 3 sensors + manned**: systems for enemy ships.
//! - **Slug crewmember**: crew locations for enemy ships.

use crate::{
    nav::{Cell, NavLocation},
    projectiles::RoomTarget,
    ship::SystemId,
    weapon::Weapon,
    Crew,
};
use bevy::{ecs::entity::MapEntities, prelude::*};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Identifies the [`IntelPackage`] for this ship.
#[derive(Component, Serialize, Deserialize)]
pub struct ShipIntel {
    pub basic: BasicIntel,
    pub crew_vision: Entity,
    pub interior: Entity,
    pub weapon_charge: Entity,
    pub systems: Entity,
}

impl MapEntities for ShipIntel {
    fn map_entities<M: EntityMapper>(&mut self, entity_mapper: &mut M) {
        self.crew_vision = entity_mapper.map_entity(self.crew_vision);
        self.interior = entity_mapper.map_entity(self.interior);
        self.weapon_charge = entity_mapper.map_entity(self.weapon_charge);
        self.systems = entity_mapper.map_entity(self.systems);
    }
}

/// Holds all the information about a ship that's visible even without functioning sensors.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BasicIntel {
    pub ship_type: usize,
    /// The ship's maximum hull integrity. This should probably move to a `ShipType` class similar
    /// to how weapons are set up. Also crew race.
    pub max_hull: usize,
    /// Current hull integrity.
    pub hull: usize,
    /// Location of each ship system, if present. If no entry for a given [`SystemId`] exists, it
    /// means the system is not installed on the ship.
    pub system_locations: HashMap<SystemId, usize>,
    /// Basic shield status if the system is installed.
    pub shields: Option<ShieldIntel>,
    /// Damage intel for engines if the system is installed.
    pub engines: Option<SystemDamageIntel>,
    /// Basic weapons status if the system is installed.
    pub weapons: Option<WeaponsIntel>,
}

/// Includes everything own crew are able to see. Drones (including hacking drones when powered) and
/// bombs count towards this as well.
#[derive(Component, Serialize, Deserialize)]
pub struct CrewVisionIntel;

#[derive(Component, Serialize, Deserialize, Debug)]
pub struct InteriorIntel {
    pub rooms: Vec<RoomIntel>,
    pub cells: Vec<CellIntel>,
}

/// Information available with vision of a room, including oxygen level and full intel of
/// all present crew.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RoomIntel {
    pub crew: Vec<CrewIntel>,
    pub oxygen: f32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CrewIntel {
    pub name: String,
    pub nav_status: CrewNavIntel,
    pub health: f32,
    pub max_health: f32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum CrewNavIntel {
    At(Cell),
    Navigating(NavLocation),
}

/// Information available with vision of a cell. Maybe should be lumped in with `RoomIntel`.
#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct CellIntel {
    pub on_fire: bool,
    pub breached: bool,
}

#[derive(Component, Serialize, Deserialize)]
pub struct WeaponChargeIntel {
    /// Stores the current charge level for each weapon. Max charge level should be read from
    /// [`BasicIntel`].
    pub levels: Vec<f32>,
}

/// This component identifies a player's ship and contains intel only they can see like targeting,
/// FTL drive status and inventory.
#[derive(Component, Serialize, Deserialize)]
pub struct SelfIntel {
    /// Points to the entity controlled by the player this component gets replicated to.
    pub ship: Entity,
    pub max_power: usize,
    pub free_power: usize,
    pub missiles: usize,
    pub weapon_targets: Vec<Option<RoomTarget>>,
    pub crew: Vec<Crew>,
}

impl MapEntities for SelfIntel {
    fn map_entities<M: EntityMapper>(&mut self, entity_mapper: &mut M) {
        self.ship = entity_mapper.map_entity(self.ship);
        for target in &mut self.weapon_targets {
            if let Some(target) = target {
                target.map_entities(entity_mapper);
            }
        }
    }
}

#[derive(Component, Serialize, Deserialize, Deref)]
pub struct SystemsIntel(pub HashMap<SystemId, SystemIntel>);

#[derive(Serialize, Deserialize)]
pub struct SystemIntel {
    pub upgrade_level: usize,
    pub damage: usize,
    pub current_power: usize,
    /// See [`SystemStatus::damage_progress`](crate::systems::SystemStatus::damage_progress).
    pub damage_progress: f32,
}

/// Basic damage intel for a system. Even players without functioning sensors can see basic system
/// information such as whether a system is damaged or destroyed.
#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub enum SystemDamageIntel {
    Undamaged,
    Damaged,
    Destroyed,
}

/// Basic shield system status, comprising layer/charge status and basic system damage state.
#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct ShieldIntel {
    /// The *current* max number of layers this shield will charge to. This doesn't necessarily
    /// relate to the system's upgrade level, only the amount of power in the system. For example,
    /// if the shield system is upgraded to level 8 but only has 5 power, `max_layers` will be 2 and
    /// will look the same as if the upgrade level and power were 4.
    pub max_layers: usize,
    /// Current number of shield layers around the ship.
    pub layers: usize,
    /// Current charge level of the next shield layer.
    pub charge: f32,
    /// Basic system damage level.
    pub damage: SystemDamageIntel,
}

/// Basic weapons system status, composed of individual weapon status and basic system damage state.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WeaponsIntel {
    pub weapons: Vec<WeaponIntel>,
    pub damage: SystemDamageIntel,
}

/// Basic weapon status for a single weapon, composed of weapon type and power state.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WeaponIntel {
    pub weapon: Weapon,
    pub powered: bool,
}
