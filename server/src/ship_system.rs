use crate::{
    engines::Engines, oxygen::Oxygen, reactor::Reactor, shields::Shields, weapons::Weapons,
};
use common::{
    intel::{SystemDamageIntel, SystemIntel},
    ship::SystemId,
};

#[derive(Clone, Debug, Default)]
pub struct ShipSystems {
    pub shields: Option<Shields>,
    pub weapons: Option<Weapons>,
    pub engines: Option<Engines>,
    pub oxygen: Option<Oxygen>,
}

impl ShipSystems {
    pub fn system(&self, system: SystemId) -> Option<&dyn ShipSystem> {
        match system {
            SystemId::Shields => self.shields.as_ref().map(|x| x as &dyn ShipSystem),
            SystemId::Weapons => self.weapons.as_ref().map(|x| x as &dyn ShipSystem),
            SystemId::Engines => self.engines.as_ref().map(|x| x as &dyn ShipSystem),
            SystemId::Oxygen => self.oxygen.as_ref().map(|x| x as &dyn ShipSystem),
        }
    }

    pub fn system_mut(&mut self, system: SystemId) -> Option<&mut dyn ShipSystem> {
        match system {
            SystemId::Shields => self.shields.as_mut().map(|x| x as &mut dyn ShipSystem),
            SystemId::Weapons => self.weapons.as_mut().map(|x| x as &mut dyn ShipSystem),
            SystemId::Engines => self.engines.as_mut().map(|x| x as &mut dyn ShipSystem),
            SystemId::Oxygen => self.oxygen.as_mut().map(|x| x as &mut dyn ShipSystem),
        }
    }

    pub fn install(&mut self, system: SystemId) {
        match system {
            SystemId::Shields => {
                self.shields = Some(Default::default());
            }
            SystemId::Weapons => {
                self.weapons = Some(Default::default());
            }
            SystemId::Engines => {
                self.engines = Some(Default::default());
            }
            SystemId::Oxygen => {
                self.oxygen = Some(Default::default());
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SystemStatus {
    upgrade_level: usize,
    damage: usize,
    /// Current progress in either damaging or repairing a system. A positive number means enemy
    /// crew are trying to break the system, and negative means it's being repaired. Once it reaches
    /// 1 or -1, it resets and `damage` is incremented or decremented. If enemy crew leave the room,
    /// positive values are reset to zero, and if all friendly crew leave, negative values are.
    damage_progress: f32,
}

impl SystemStatus {
    pub fn max_power(&self) -> usize {
        self.upgrade_level - self.damage
    }
}

impl Default for SystemStatus {
    fn default() -> Self {
        Self {
            upgrade_level: 1,
            damage: 0,
            damage_progress: 0.0,
        }
    }
}

/// Stuff that a [`ShipSystem`] impl might need to know to properly implement all trait items.
pub struct PowerContext {
    pub missiles: usize,
}

pub trait ShipSystem {
    fn system_status(&self) -> SystemStatus;
    fn system_status_mut(&mut self) -> &mut SystemStatus;
    fn current_power(&self) -> usize;
    fn add_power(&mut self, reactor: &mut Reactor, context: PowerContext);
    fn remove_power(&mut self, reactor: &mut Reactor);

    fn intel(&self) -> SystemIntel {
        let status = self.system_status();
        SystemIntel {
            upgrade_level: status.upgrade_level,
            damage: status.damage,
            current_power: self.current_power(),
            damage_progress: status.damage_progress,
        }
    }

    fn damage_intel(&self) -> SystemDamageIntel {
        if self.damage() == self.upgrade_level() {
            SystemDamageIntel::Destroyed
        } else if self.damage() == 0 {
            SystemDamageIntel::Undamaged
        } else {
            SystemDamageIntel::Damaged
        }
    }

    fn damage_system(&mut self, amount: usize, reactor: &mut Reactor) {
        let SystemStatus {
            upgrade_level,
            damage,
            damage_progress,
        } = self.system_status_mut();
        // Cap max damage to our upgrade level
        let actual_amount = amount.min(*upgrade_level - *damage);
        // Apply damage
        *damage += actual_amount;
        // Compute new max power
        let new_max = *upgrade_level - *damage;
        // Cancel any current sabotage
        if new_max == 0 {
            *damage_progress = damage_progress.min(0.0);
        }
        // Reduce power until we're back within our system power budget
        while self.current_power() > new_max {
            self.remove_power(reactor);
        }
    }

    fn repair_system(&mut self, amount: usize) {
        let SystemStatus {
            damage,
            damage_progress,
            ..
        } = self.system_status_mut();
        // Cap max repair to our current damage level
        *damage = damage.saturating_sub(amount);
        // Cancel any current repair
        if *damage == 0 {
            *damage_progress = damage_progress.max(0.0);
        }
    }

    fn _crew_damage(&mut self, amount: f32, reactor: &mut Reactor) {
        let damage_progress = &mut self.system_status_mut().damage_progress;
        *damage_progress += amount;
        if *damage_progress >= 1.0 {
            *damage_progress = 0.0;
            self.damage_system(1, reactor);
            // TODO upgrade crew combat skill
        }
    }

    fn crew_repair(&mut self, amount: f32) {
        let damage_progress = &mut self.system_status_mut().damage_progress;
        *damage_progress -= amount;
        if *damage_progress <= -1.0 {
            *damage_progress = 0.0;
            self.repair_system(1);
            // TODO upgrade crew repair skill
        }
    }

    fn cancel_repair(&mut self) {
        self.system_status_mut().damage_progress = 0.0;
    }

    fn upgrade(&mut self) {
        let SystemStatus { upgrade_level, .. } = self.system_status_mut();
        *upgrade_level += 1;
    }

    fn upgrade_level(&self) -> usize {
        let SystemStatus { upgrade_level, .. } = self.system_status();
        upgrade_level
    }

    fn damage(&self) -> usize {
        let SystemStatus { damage, .. } = self.system_status();
        damage
    }
}

pub fn boring_add_power(
    max: usize,
    current_power: &mut usize,
    reactor: &mut Reactor,
    system: SystemId,
) {
    if *current_power + 1 > max {
        eprintln!("Can't add power to {system}, system power is already at max.");
        return;
    }
    let Some(new_available) = reactor.available.checked_sub(1) else {
        eprintln!("Can't add power to {system}, no available reactor power.");
        return;
    };
    reactor.available = new_available;
    *current_power += 1;
}

pub fn boring_remove_power(current_power: &mut usize, reactor: &mut Reactor, system: SystemId) {
    if *current_power == 0 {
        eprintln!("Can't remove power from {system}, system power is already zero.");
        return;
    }
    reactor.available += 1;
    *current_power -= 1;
}
