pub mod events;
pub mod projectiles;
mod replicate_resource;
mod systems;

use std::{collections::HashSet, ops::Deref, time::Duration};

use bevy::{ecs::entity::MapEntities, prelude::*};
use bevy_replicon::prelude::*;
use events::{
    AdjustPower,
    MoveWeapon,
    SetAutofire,
    SetCrewGoal,
    // adjust_power, set_autofire, set_projectile_weapon_target, weapon_power,
    SetProjectileWeaponTarget,
    WeaponPower,
};
use replicate_resource::ReplicateResExt;
use serde::{Deserialize, Serialize};
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

use projectiles::*;
pub use system::{Engines, Reactor, Shields, Weapons};
pub use systems::{ShipSystem, SystemStatus};

pub const PROTOCOL_ID: u64 = 1;

pub fn protocol_plugin(app: &mut App) {
    app.replicate_resource::<ReadyState>();

    app.replicate_mapped::<Ship>();
    app.replicate_mapped::<ShipIntel>();
    app.replicate_mapped::<IntelPackage>();
    app.replicate::<BasicIntel>();
    app.replicate::<Traversal>();
    app.replicate::<WeaponDamage>();
    app.replicate::<NeedsDodgeTest>();
    app.replicate::<Name>();
    app.replicate::<Dead>();

    app.add_client_event::<PlayerReady>(ChannelKind::Ordered);

    app.add_client_event::<AdjustPower>(ChannelKind::Ordered);
    app.add_client_event::<WeaponPower>(ChannelKind::Ordered);
    app.add_mapped_client_event::<SetProjectileWeaponTarget>(ChannelKind::Ordered);
    app.add_client_event::<MoveWeapon>(ChannelKind::Ordered);
    app.add_mapped_client_event::<SetCrewGoal>(ChannelKind::Ordered);
    app.add_client_event::<SetAutofire>(ChannelKind::Ordered);
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
    // pub weapon_charge: Entity,
    // pub full: Entity,
}

impl MapEntities for IntelPackage {
    fn map_entities<M: EntityMapper>(&mut self, entity_mapper: &mut M) {
        self.basic = entity_mapper.map_entity(self.basic);
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct ShieldIntel {
    pub max_layers: usize,
    pub layers: usize,
    pub charge: f32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WeaponIntel {
    pub weapon: Weapon,
    pub powered: bool,
}

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
            }),
            weapons: ship.systems.weapons().map(|weapons| {
                weapons
                    .weapons()
                    .iter()
                    .map(|x| WeaponIntel {
                        weapon: x.weapon.clone(),
                        powered: x.is_powered(),
                    })
                    .collect()
            }),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub enum PowerDir {
    Request,
    Remove,
}

#[derive(Serialize, Deserialize, EnumIter, Debug, Clone, Copy)]
pub enum SystemId {
    Shields,
    Weapons,
    Engines,
}

impl std::fmt::Display for SystemId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Shields => write!(f, "shields"),
            Self::Weapons => write!(f, "weapons"),
            Self::Engines => write!(f, "engines"),
        }
    }
}

#[derive(Resource, Serialize, Deserialize, Debug, Clone)]
pub enum ReadyState {
    AwaitingClients { ready_clients: HashSet<ClientId> },
    Starting { countdown: Duration },
}

impl Default for ReadyState {
    fn default() -> Self {
        Self::AwaitingClients {
            ready_clients: default(),
        }
    }
}

#[derive(Event, Serialize, Deserialize, Default, Clone, Copy)]
pub struct PlayerReady;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Room {
    _cells: Vec<usize>,
}

#[derive(Component, Serialize, Deserialize, Debug, Default)]
pub struct Dead;

#[derive(Component, Serialize, Deserialize, Clone, Debug)]
pub struct Ship {
    pub reactor: Reactor,
    pub systems: ShipSystems,
    pub max_hull: usize,
    pub damage: usize,
    rooms: Vec<Room>,
}

impl MapEntities for Ship {
    fn map_entities<M: EntityMapper>(&mut self, entity_mapper: &mut M) {
        self.systems.map_entities(entity_mapper);
    }
}

impl Ship {
    pub fn new() -> Self {
        Self {
            reactor: Reactor::new(0),
            systems: default(),
            max_hull: 30,
            damage: 0,
            rooms: vec![
                Room {
                    _cells: vec![0, 1, 2, 3],
                },
                Room { _cells: vec![4, 5] },
                Room { _cells: vec![6, 7] },
            ],
        }
    }

    pub fn update_ships(mut ships: Query<&mut Ship, Without<Dead>>, mut commands: Commands) {
        for mut ship in &mut ships {
            if let Some(shields) = ship.systems.shields_mut() {
                shields.charge_shield();
            }
            if let Some(weapons) = ship.systems.weapons_mut() {
                for projectile in weapons.charge_and_fire_weapons() {
                    commands.spawn(projectile);
                }
            }
        }
    }

    pub fn install_shields(&mut self, room: usize) {
        if self.systems.system_in_room(room).is_some() {
            eprintln!("Can't install shields in room {room}, room is already occupied.");
            return;
        }
        if self.systems.shields.is_some() {
            eprintln!("Can't install shields on ship, a shields module is already installed.");
            return;
        }
        self.systems.shields = Some((default(), room));
    }

    pub fn install_engines(&mut self, room: usize) {
        if self.systems.system_in_room(room).is_some() {
            eprintln!("Can't install engines in room {room}, room is already occupied.");
            return;
        }
        if self.systems.engines.is_some() {
            eprintln!("Can't install engines on ship, an engines module is already installed.");
            return;
        }
        self.systems.engines = Some((default(), room));
    }

    pub fn install_weapons(&mut self, room: usize) {
        if self.systems.system_in_room(room).is_some() {
            eprintln!("Can't install engines in room {room}, room is already occupied.");
            return;
        }
        if self.systems.weapons.is_some() {
            eprintln!("Can't install weapons on ship, a weapons module is already installed.");
            return;
        }
        self.systems.weapons = Some((default(), room));
    }

    pub fn request_power(&mut self, system: SystemId) {
        let Some(system) = self.systems.system_mut(system) else {
            eprintln!("Can't add power to {system}, system not installed.");
            return;
        };
        system.add_power(&mut self.reactor);
    }

    pub fn remove_power(&mut self, system: SystemId) {
        let Some(system) = self.systems.system_mut(system) else {
            eprintln!("Can't remove power from {system}, system not installed.");
            return;
        };
        system.remove_power(&mut self.reactor);
    }

    pub fn power_weapon(&mut self, index: usize) {
        let Some(weapons) = self.systems.weapons_mut() else {
            eprintln!("Can't power weapon, weapons system not installed.");
            return;
        };
        weapons.power_weapon(index, &mut self.reactor);
    }

    pub fn depower_weapon(&mut self, index: usize) {
        let Some(weapons) = self.systems.weapons_mut() else {
            eprintln!("Can't depower weapon, weapons system not installed.");
            return;
        };
        weapons.depower_weapon(index, &mut self.reactor);
    }

    pub fn set_projectile_weapon_target(
        &mut self,
        weapon_index: usize,
        target: Option<ProjectileTarget>,
        targeting_self: bool,
    ) {
        let Some(weapons) = self.systems.weapons_mut() else {
            eprintln!("Can't set weapon target, weapons system notinstalled.");
            return;
        };
        weapons.set_projectile_weapon_target(weapon_index, target, targeting_self);
    }

    pub fn set_autofire(&mut self, autofire: bool) {
        let Some(weapons) = self.systems.weapons_mut() else {
            eprintln!("Can't set autofire, weapons system not installed.");
            return;
        };
        weapons.autofire = autofire;
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct ShipSystems {
    shields: Option<(Shields, usize)>,
    engines: Option<(Engines, usize)>,
    weapons: Option<(Weapons, usize)>,
}

impl MapEntities for ShipSystems {
    fn map_entities<M: EntityMapper>(&mut self, entity_mapper: &mut M) {
        if let Some((weapons, _)) = &mut self.weapons {
            weapons.map_entities(entity_mapper);
        }
    }
}

impl ShipSystems {
    // Finds the system housed by `room` (there may not be a system in that room).
    pub fn system_in_room(&self, room: usize) -> Option<SystemId> {
        SystemId::iter().find(|&system| self.system_room(system) == Some(room))
    }

    fn system_room(&self, system: SystemId) -> Option<usize> {
        match system {
            SystemId::Shields => self.shields.as_ref().map(|(_, x)| *x),
            SystemId::Engines => self.engines.as_ref().map(|(_, x)| *x),
            SystemId::Weapons => self.weapons.as_ref().map(|(_, x)| *x),
        }
    }

    pub fn system_mut(&mut self, system: SystemId) -> Option<&mut dyn ShipSystem> {
        match system {
            SystemId::Shields => self.shields.as_mut().map(|(x, _)| x as &mut dyn ShipSystem),
            SystemId::Weapons => self.weapons.as_mut().map(|(x, _)| x as &mut dyn ShipSystem),
            SystemId::Engines => self.engines.as_mut().map(|(x, _)| x as &mut dyn ShipSystem),
        }
    }

    pub fn shields(&self) -> Option<&Shields> {
        self.shields.as_ref().map(|(x, _)| x)
    }

    pub fn engines(&self) -> Option<&Engines> {
        self.engines.as_ref().map(|(x, _)| x)
    }

    pub fn weapons(&self) -> Option<&Weapons> {
        self.weapons.as_ref().map(|(x, _)| x)
    }

    pub fn shields_mut(&mut self) -> Option<&mut Shields> {
        self.shields.as_mut().map(|(x, _)| x)
    }

    pub fn engines_mut(&mut self) -> Option<&mut Engines> {
        self.engines.as_mut().map(|(x, _)| x)
    }

    pub fn weapons_mut(&mut self) -> Option<&mut Weapons> {
        self.weapons.as_mut().map(|(x, _)| x)
    }
}

mod system {
    use super::*;

    #[derive(Serialize, Deserialize, Clone, Debug)]
    pub struct Reactor {
        pub upgrade_level: usize,
        pub available: usize,
    }

    impl Reactor {
        pub fn new(upgrade_level: usize) -> Self {
            Self {
                upgrade_level,
                available: upgrade_level,
            }
        }

        pub fn upgrade(&mut self) {
            self.upgrade_level += 1;
            self.available += 1;
        }
    }

    #[derive(Serialize, Deserialize, Debug, Default, Clone)]
    pub struct Shields {
        status: SystemStatus,
        /// Current reactor power allocated to shields. `layers` will never
        /// exceed `current_power / 2`.
        current_power: usize,
        /// Current number of shield rings.
        pub layers: usize,
        /// Current progress toward recovering the next shield layer.
        pub charge: f32,
    }

    impl Shields {
        pub fn charge_shield(&mut self) {
            let target = self.current_power / 2;
            if self.layers > target {
                self.layers = target;
            }
            if self.layers < target {
                self.charge += 0.01;
            } else {
                self.charge = 0.0;
            }
            if self.charge >= 1.0 {
                self.charge = 0.0;
                self.layers += 1;
            }
        }

        pub fn current_power(&self) -> usize {
            self.current_power
        }

        pub fn max_layers(&self) -> usize {
            self.current_power / 2
        }
    }

    impl ShipSystem for Shields {
        fn system_status(&self) -> SystemStatus {
            self.status
        }

        fn system_status_mut(&mut self) -> &mut SystemStatus {
            &mut self.status
        }

        fn current_power(&self) -> usize {
            self.current_power
        }

        fn add_power(&mut self, reactor: &mut Reactor) {
            // Divide then multiply by two to truncate odd numbers to latest even
            let next_level = (self.current_power + 2) / 2 * 2;
            if next_level > self.status.max_power() {
                eprintln!("Can't add power to shields, system power would exceed upgrade level.");
                return;
            }
            let diff = next_level - self.current_power;
            let Some(new_available) = reactor.available.checked_sub(diff) else {
                eprintln!("Can't add power to shields, available reactor power is insufficient.");
                return;
            };
            reactor.available = new_available;
            self.current_power += diff;
        }

        fn remove_power(&mut self, reactor: &mut Reactor) {
            if self.current_power == 0 {
                eprintln!("Can't remove power from shields, system power is already zero.");
                return;
            }
            let prev_level = (self.current_power - 1) / 2 * 2;
            let diff = self.current_power - prev_level;
            reactor.available += diff;
            self.current_power -= diff;
        }
    }

    #[derive(Serialize, Deserialize, Debug, Default, Clone)]
    pub struct Engines {
        status: SystemStatus,
        current_power: usize,
    }

    impl Engines {
        pub fn dodge_chance(&self) -> usize {
            5 * self.current_power
        }

        pub fn current_power(&self) -> usize {
            self.current_power
        }
    }

    impl ShipSystem for Engines {
        fn system_status(&self) -> SystemStatus {
            self.status
        }

        fn system_status_mut(&mut self) -> &mut SystemStatus {
            &mut self.status
        }

        fn current_power(&self) -> usize {
            self.current_power
        }

        fn add_power(&mut self, reactor: &mut Reactor) {
            if self.current_power + 1 > self.status.max_power() {
                eprintln!("Can't add power to engines, system power is already at max.");
                return;
            }
            let Some(new_available) = reactor.available.checked_sub(1) else {
                eprintln!("Can't add power to engines, no available reactor power.");
                return;
            };
            reactor.available = new_available;
            self.current_power += 1;
        }

        fn remove_power(&mut self, reactor: &mut Reactor) {
            if self.current_power == 0 {
                eprintln!("Can't remove power from engines, system power is already zero.");
                return;
            }
            reactor.available += 1;
            self.current_power -= 1;
        }
    }

    #[derive(Serialize, Deserialize, Debug, Default, Clone)]
    pub struct Weapons {
        status: SystemStatus,
        entries: Vec<WeaponEntry>,
        pub autofire: bool,
        missiles: usize,
    }

    impl MapEntities for Weapons {
        fn map_entities<M: EntityMapper>(&mut self, entity_mapper: &mut M) {
            for entry in &mut self.entries {
                entry.map_entities(entity_mapper);
            }
        }
    }

    impl Weapons {
        pub fn charge_and_fire_weapons(&mut self) -> impl Iterator<Item = ProjectileBundle> + '_ {
            self.entries
                .iter_mut()
                .filter_map(|x| x.charge_and_fire(&mut self.missiles, self.autofire))
        }

        pub fn weapons(&self) -> &Vec<WeaponEntry> {
            &self.entries
        }

        pub fn current_power(&self) -> usize {
            self.entries
                .iter()
                .filter(|x| x.is_powered())
                .fold(0, |x, y| x + y.weapon.power)
        }

        pub fn missile_count(&self) -> usize {
            self.missiles
        }

        pub fn add_missiles(&mut self, count: usize) {
            self.missiles += count;
        }

        pub fn _remove_missiles(&mut self, count: usize) -> usize {
            let diff = self.missiles.min(count);
            self.missiles -= diff;
            diff
        }

        pub fn power_weapon(&mut self, index: usize, reactor: &mut Reactor) {
            let used_power = self.current_power();
            let Some(weapon) = self.entries.get_mut(index) else {
                eprintln!("Can't power nonexistent weapon at index {index}.");
                return;
            };
            if weapon.is_powered() {
                eprintln!("Can't power weapon at index {index}, weapon is already powered.");
                return;
            }
            let requested_power = weapon.weapon.power;
            if used_power + requested_power > self.status.max_power() {
                eprintln!("Can't add power to weapons, system power would exceed upgrade level.");
                return;
            }
            if weapon.weapon.uses_missile && self.missiles == 0 {
                eprintln!("Can't power weapon, no missiles in supply.");
                return;
            }
            let Some(new_reactor) = reactor.available.checked_sub(requested_power) else {
                eprintln!("Can't add power to weapons, available reactor power is insufficient.");
                return;
            };
            reactor.available = new_reactor;
            weapon.status = ProjectileWeaponStatus::Powered { target: None };
        }

        pub fn depower_weapon(&mut self, index: usize, reactor: &mut Reactor) {
            let Some(weapon) = self.entries.get_mut(index) else {
                eprintln!(
                    "Can't depower nonexistent weapon at index
    {index}."
                );
                return;
            };
            if !weapon.is_powered() {
                eprintln!("Can't depower weapon at index {index}, weapon is not powered.");
                return;
            }
            reactor.available += weapon.weapon.power;
            weapon.status = ProjectileWeaponStatus::Unpowered;
        }

        pub fn set_projectile_weapon_target(
            &mut self,
            weapon_index: usize,
            target: Option<ProjectileTarget>,
            targeting_self: bool,
        ) {
            let Some(weapon) = self.entries.get_mut(weapon_index) else {
                eprintln!("Can't set weapon target, no weapon in slot {weapon_index}.");
                return;
            };
            weapon.set_target(target, targeting_self);
        }

        pub fn add_weapon(&mut self, index: usize, weapon: Weapon) {
            if index > self.entries.len() {
                eprintln!("Can't add weapon at index {index}, not enough weapons installed.");
                return;
            }
            self.entries.insert(index, WeaponEntry::new(weapon));
        }

        pub fn _remove_weapon(
            &mut self,
            index: usize,
            reactor: &mut Reactor,
        ) -> Result<Weapon, ()> {
            if index >= self.entries.len() {
                eprintln!("Can't remove weapon, no weapon at slot {index}.");
                return Err(());
            }
            if self.entries[index].is_powered() {
                self.depower_weapon(index, reactor);
            }
            Ok(self.entries.remove(index).weapon)
        }

        // pub fn shift_weapon_right(&mut self, index: usize) {
        //     let element = self.entries.remove(index);
        //     self.entries.insert(index + 1, element);
        // }

        // pub fn shift_weapon_left(&mut self, index: usize) {
        //     let element = self.entries.remove(index);
        //     self.entries.insert(index - 1, element);
        // }
    }

    impl ShipSystem for Weapons {
        fn system_status(&self) -> SystemStatus {
            self.status
        }

        fn system_status_mut(&mut self) -> &mut SystemStatus {
            &mut self.status
        }

        fn current_power(&self) -> usize {
            self.current_power()
        }

        fn add_power(&mut self, reactor: &mut Reactor) {
            let Some(next_depowered) = self.entries.iter().position(|x| !x.is_powered()) else {
                eprintln!("Can't increase power to weapons, all weapons are powered.");
                return;
            };
            self.power_weapon(next_depowered, reactor);
        }

        fn remove_power(&mut self, reactor: &mut Reactor) {
            let Some(next_powered) = self.entries.iter().rev().position(|x| x.is_powered()) else {
                eprintln!("Can't decrease power to weapons, no weapons are powered.");
                return;
            };
            let next_powered = self.entries.len() - 1 - next_powered;
            self.depower_weapon(next_powered, reactor);
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub enum ProjectileWeaponStatus {
    Unpowered,
    Powered { target: Option<ProjectileTarget> },
}

impl MapEntities for ProjectileWeaponStatus {
    fn map_entities<M: EntityMapper>(&mut self, entity_mapper: &mut M) {
        if let Self::Powered {
            target: Some(target),
        } = self
        {
            target.map_entities(entity_mapper);
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WeaponEntry {
    pub weapon: Weapon,
    status: ProjectileWeaponStatus,
    pub charge: f32,
}

impl MapEntities for WeaponEntry {
    fn map_entities<M: EntityMapper>(&mut self, entity_mapper: &mut M) {
        self.status.map_entities(entity_mapper);
    }
}

impl WeaponEntry {
    pub fn new(weapon: Weapon) -> Self {
        Self {
            weapon,
            status: ProjectileWeaponStatus::Unpowered,
            charge: 0.0,
        }
    }

    pub fn is_powered(&self) -> bool {
        matches!(self.status, ProjectileWeaponStatus::Powered { .. })
    }

    pub fn target(&self) -> Option<ProjectileTarget> {
        if let ProjectileWeaponStatus::Powered { target } = self.status {
            target
        } else {
            None
        }
    }

    pub fn set_target(&mut self, new_target: Option<ProjectileTarget>, targeting_self: bool) {
        let ProjectileWeaponStatus::Powered { target } = &mut self.status else {
            eprintln!("Can't set weapon target, weapon is unpowered.");
            return;
        };
        if targeting_self && !self.weapon.can_target_self {
            eprintln!("Can't set weapon target, weapon cannot target self.");
            return;
        }
        *target = new_target;
    }

    #[must_use]
    pub fn charge_and_fire(
        &mut self,
        missiles: &mut usize,
        autofire: bool,
    ) -> Option<ProjectileBundle> {
        if let ProjectileWeaponStatus::Powered { target } = &mut self.status {
            self.charge = (self.charge + 1.0 / 64.0).min(self.weapon.charge_time);
            if self.charge == self.weapon.charge_time {
                if let Some(target_room) = target.take() {
                    self.charge = 0.0;
                    if self.weapon.uses_missile {
                        *missiles -= 1;
                    }
                    let projectile = ProjectileBundle {
                        replicated: default(),
                        damage: WeaponDamage(self.weapon.damage),
                        target: target_room,
                        traversal_speed: TraversalSpeed(self.weapon.shot_speed),
                        traversal_progress: default(),
                        needs_dodge_test: default(),
                        shield_pierce: ShieldPierce(self.weapon.shield_pierce),
                    };
                    if autofire {
                        *target = Some(target_room);
                    }
                    return Some(projectile);
                }
            }
        } else {
            self.charge = (self.charge - 6.0 / 64.0).max(0.0);
        }
        None
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Weapon(pub usize);

impl Deref for Weapon {
    type Target = WeaponType;

    fn deref(&self) -> &Self::Target {
        &WEAPONS[self.0]
    }
}

#[derive(Debug, Clone, Copy)]
pub struct WeaponType {
    pub name: &'static str,
    pub damage: usize,
    pub power: usize,
    pub charge_time: f32,
    pub shot_speed: f32,
    pub shield_pierce: usize,
    pub uses_missile: bool,
    pub can_target_self: bool,
}

const WEAPONS: [WeaponType; 2] = [
    WeaponType {
        name: "Heavy Laser",
        damage: 2,
        power: 1,
        charge_time: 9.0,
        shot_speed: 1.0,
        shield_pierce: 0,
        uses_missile: false,
        can_target_self: false,
    },
    WeaponType {
        name: "Hermes Missiles",
        damage: 3,
        power: 3,
        charge_time: 14.0,
        shot_speed: 0.6,
        shield_pierce: 5,
        uses_missile: true,
        can_target_self: false,
    },
];
