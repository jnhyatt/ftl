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
    ship::SystemId,
    weapon::{WeaponId, WeaponTarget},
    Crew, DoorState,
};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Identifies the intel chunks for this ship. The entity this component is attached to is the
/// canonical ship entity and is always replicated to all clients. [`BasicIntel`] is stuff that you
/// can see even without functioning sensors, so it's included here. The other intel chunks are
/// separate entities pointed to by this component. They may or may not be replicated to clients
/// depending on sensor status.
#[derive(Component, Serialize, Deserialize)]
pub struct ShipIntel {
    /// Since [`ShipIntel`] is replicated to all clients unconditionally, everything in this struct
    /// is always visible to all clients. Therefore, we put basic intel that everyone sees in here.
    /// This is typically just stuff like hull integrity, system locations, and basic shield status
    /// that's visible just by looking at the ship.
    pub basic: BasicIntel,
    /// Full interior intel for this ship. This is available for a player's own ship with any sensor
    /// level, and for enemy ships with level 2 sensors or higher.
    #[entities]
    pub interior: Entity,
    /// Exact charge levels for all weapons on this ship. This is available for a player's own ship
    /// even without sensors, and for enemy ships with level 3 sensors or higher.
    #[entities]
    pub weapon_charge: Entity,
    /// Power distribution and status for all systems on this ship. This is available for a player's
    /// own ship even without sensors, and for enemy ships with level 4 sensors (fully upgraded and
    /// manned).
    #[entities]
    pub systems: Entity,
}

/// Holds all the information about a ship that's visible even without functioning sensors.
#[derive(Component, Serialize, Deserialize, Debug, Clone)]
pub struct BasicIntel {
    pub ship_type: usize,
    /// Current hull integrity.
    pub hull: usize,
    /// Location of each ship system, if present. If no entry for a given [`SystemId`] exists, it
    /// means the system is not installed on the ship. Could maybe move these to individual structs
    /// (like [`ShieldIntel`] for example) so we're not duplicate presence information (i.e. whether
    /// the system is installed affects both the presence in `system_locations` and the presence of
    /// the corresponding intel struct, and they should be kept in sync).
    pub system_locations: HashMap<SystemId, usize>,
    /// Basic shield status if the system is installed.
    pub shields: Option<ShieldIntel>,
    /// Damage intel for engines if the system is installed.
    pub engines: Option<SystemDamageIntel>,
    /// Basic weapons status if the system is installed.
    pub weapons: Option<WeaponsIntel>,
    pub oxygen: Option<SystemDamageIntel>,
    pub doors: Vec<DoorState>,
}

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
    pub race: usize,
    pub name: String,
    pub nav_status: CrewNavIntel,
    pub health: f32,
}

/// Navigation status for a crew member, either stationary at a cell or walking on a
/// [`NavSection`](crate::nav::NavSection). If a crew is navigating, this intel doesn't tell you
/// their ultimate destination, just the section they're currently traversing (plus their progress
/// along that section).
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
    /// Points to the ship entity controlled by the player this component gets replicated to.
    #[entities]
    pub ship: Entity,
    pub max_power: usize,
    pub free_power: usize,
    pub missiles: usize,
    #[entities]
    pub weapon_targets: Vec<Option<WeaponTarget>>,
    pub crew: Vec<Crew>,
    pub autofire: bool,
    pub oxygen: f32,
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
#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct WeaponIntel {
    pub weapon: WeaponId,
    pub powered: bool,
}

#[cfg(test)]
mod tests {
    use bevy::{
        ecs::entity::{EntityGeneration, EntityHashMap, EntityRow},
        prelude::*,
    };

    use crate::bullets::RoomTarget;

    use super::*;

    fn new_entity(x: u32) -> Entity {
        Entity::from_row_and_generation(
            EntityRow::new(TryFrom::<u32>::try_from(x).unwrap()),
            EntityGeneration::FIRST,
        )
    }

    #[test]
    fn test_map_room_target() {
        let [a, b] = [0, 1].map(new_entity);
        let mut target = RoomTarget { ship: a, room: 0 };
        let mut mapper = EntityHashMap::from([(a, b)]);
        <RoomTarget as bevy::ecs::entity::MapEntities>::map_entities(&mut target, &mut mapper);
        // target.map_entities(&mut mapper);
        assert_eq!(target.ship, b);
    }

    #[test]
    fn test_map_weapon_target() {
        let [a, b] = [0, 1].map(new_entity);
        let mut target = WeaponTarget::Projectile(RoomTarget { ship: a, room: 0 });
        let mut mapper = EntityHashMap::from([(a, b)]);
        <WeaponTarget as bevy::ecs::entity::MapEntities>::map_entities(&mut target, &mut mapper);
        let WeaponTarget::Projectile(target) = target else {
            unreachable!();
        };
        assert_eq!(target.ship, b);
    }

    #[test]
    fn test_map_self_intel() {
        let [a, b, c, d] = [0, 1, 2, 3].map(new_entity);
        let mut intel = SelfIntel {
            ship: a,
            max_power: 0,
            free_power: 0,
            missiles: 0,
            weapon_targets: [Some(WeaponTarget::Projectile(RoomTarget {
                ship: b,
                room: 0,
            }))]
            .into(),
            crew: [].into(),
            autofire: false,
            oxygen: 0.0,
        };
        let mut mapper = EntityHashMap::from([(a, c), (b, d)]);
        SelfIntel::map_entities(&mut intel, &mut mapper);
        assert_eq!(intel.ship, c);
        let Some(WeaponTarget::Projectile(target)) = intel.weapon_targets[0] else {
            unreachable!();
        };
        assert_eq!(target.ship, d);
    }
}
