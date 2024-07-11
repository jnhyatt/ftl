use crate::{
    reactor::Reactor,
    ship_system::{PowerContext, ShipSystem, SystemStatus},
};
use common::{projectiles::RoomTarget, Weapon, WeaponType};

#[derive(Debug, Default, Clone)]
pub struct Weapons {
    status: SystemStatus,
    entries: Vec<WeaponEntry>,
    pub autofire: bool,
}

impl Weapons {
    pub fn charge_and_fire_weapons<'a>(
        &'a mut self,
        missiles: &'a mut usize,
    ) -> impl Iterator<Item = ProjectileInfo> + 'a {
        self.entries
            .iter_mut()
            .filter_map(|x| x.charge_and_fire(missiles, self.autofire))
    }

    pub fn weapons(&self) -> &Vec<WeaponEntry> {
        &self.entries
    }

    pub fn weapons_mut(&mut self) -> impl Iterator<Item = &mut WeaponEntry> {
        self.entries.iter_mut()
    }

    pub fn current_power(&self) -> usize {
        self.entries
            .iter()
            .filter(|x| x.is_powered())
            .fold(0, |x, y| x + y.weapon.power)
    }

    pub fn power_weapon(&mut self, index: usize, missiles: usize, reactor: &mut Reactor) {
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
        if weapon.weapon.uses_missile && missiles == 0 {
            eprintln!("Can't power weapon, no missiles in supply.");
            return;
        }
        let Some(new_reactor) = reactor.available.checked_sub(requested_power) else {
            eprintln!("Can't add power to weapons, available reactor power is insufficient.");
            return;
        };
        reactor.available = new_reactor;
        weapon.status = WeaponStatus::Powered { target: None };
    }

    pub fn depower_weapon(&mut self, index: usize, reactor: &mut Reactor) {
        let Some(weapon) = self.entries.get_mut(index) else {
            eprintln!("Can't depower nonexistent weapon at index {index}.");
            return;
        };
        if !weapon.is_powered() {
            eprintln!("Can't depower weapon at index {index}, weapon is not powered.");
            return;
        }
        reactor.available += weapon.weapon.power;
        weapon.status = WeaponStatus::Unpowered;
    }

    pub fn set_projectile_weapon_target(
        &mut self,
        weapon_index: usize,
        target: Option<RoomTarget>,
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

    pub fn _remove_weapon(&mut self, index: usize, reactor: &mut Reactor) -> Result<Weapon, ()> {
        if index >= self.entries.len() {
            eprintln!("Can't remove weapon, no weapon in slot {index}.");
            return Err(());
        }
        if self.entries[index].is_powered() {
            self.depower_weapon(index, reactor);
        }
        Ok(self.entries.remove(index).weapon)
    }

    pub fn move_weapon(&mut self, index: usize, target: usize) {
        if index >= self.entries.len() {
            eprintln!("Can't move weapon, no weapon in slot {index}.");
            return;
        }
        if target > self.entries.len() - 1 {
            eprintln!("Can't move weapon, slot {target} is out of bounds.");
            return;
        }
        let element = self.entries.remove(index);
        self.entries.insert(target, element);
    }
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

    fn add_power(&mut self, reactor: &mut Reactor, context: PowerContext) {
        let Some(next_depowered) = self.entries.iter().position(|x| !x.is_powered()) else {
            eprintln!("Can't increase power to weapons, all weapons are powered.");
            return;
        };
        self.power_weapon(next_depowered, context.missiles, reactor);
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

#[derive(Debug, Clone)]
pub struct WeaponEntry {
    pub weapon: Weapon,
    status: WeaponStatus<RoomTarget>,
    pub charge: f32,
}

impl WeaponEntry {
    pub fn new(weapon: Weapon) -> Self {
        Self {
            weapon,
            status: WeaponStatus::Unpowered,
            charge: 0.0,
        }
    }

    pub fn is_powered(&self) -> bool {
        matches!(self.status, WeaponStatus::Powered { .. })
    }

    pub fn target(&self) -> Option<RoomTarget> {
        if let WeaponStatus::Powered { target } = self.status {
            target
        } else {
            None
        }
    }

    pub fn set_target(&mut self, new_target: Option<RoomTarget>, targeting_self: bool) {
        let WeaponStatus::Powered { target } = &mut self.status else {
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
    ) -> Option<ProjectileInfo> {
        if let WeaponStatus::Powered { target } = &mut self.status {
            self.charge = (self.charge + 1.0 / 64.0).min(self.weapon.charge_time);
            if self.charge == self.weapon.charge_time {
                if let Some(target_room) = target.take() {
                    self.charge = 0.0;
                    if self.weapon.uses_missile {
                        *missiles -= 1;
                    }
                    if autofire {
                        *target = Some(target_room);
                    }
                    return Some(ProjectileInfo {
                        weapon: *self.weapon,
                        target: target_room,
                        count: self.weapon.volley_size,
                    });
                }
            }
        } else {
            self.charge = (self.charge - 6.0 / 64.0).max(0.0);
        }
        None
    }
}

#[derive(Debug, Clone, Copy)]
pub enum WeaponStatus<Target> {
    Unpowered,
    Powered { target: Option<Target> },
}

pub struct ProjectileInfo {
    pub weapon: WeaponType,
    pub target: RoomTarget,
    pub count: usize,
}
