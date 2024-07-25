use crate::{
    reactor::Reactor,
    ship_system::{PowerContext, ShipSystem, SystemStatus},
};
use common::{
    bullets::{BeamTarget, RoomTarget},
    weapon::{BeamWeapon, ProjectileWeapon, Weapon, WeaponId, WeaponTarget, Weaponlike},
};

#[derive(Debug, Default)]
pub struct Weapons {
    status: SystemStatus,
    entries: Vec<WeaponEntry>,
    pub autofire: bool,
}

impl Weapons {
    pub fn charge_and_fire_weapons<'a>(
        &'a mut self,
        missiles: &'a mut usize,
    ) -> impl Iterator<Item = Volley> + 'a {
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
            .fold(0, |x, y| x + y.weapon().common().power)
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
        let requested_power = weapon.weapon().common().power;
        if used_power + requested_power > self.status.max_power() {
            eprintln!("Can't add power to weapons, system power would exceed upgrade level.");
            return;
        }
        if weapon.weapon().uses_missile() && missiles == 0 {
            eprintln!("Can't power weapon, no missiles in supply.");
            return;
        }
        let Some(new_reactor) = reactor.available.checked_sub(requested_power) else {
            eprintln!("Can't add power to weapons, available reactor power is insufficient.");
            return;
        };
        reactor.available = new_reactor;
        weapon.add_power();
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

        reactor.available += weapon.weapon().common().power;
        weapon.remove_power();
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
        weapon.set_room_target(target, targeting_self);
    }

    pub fn set_beam_weapon_target(&mut self, weapon_index: usize, target: Option<BeamTarget>) {
        let Some(weapon) = self.entries.get_mut(weapon_index) else {
            eprintln!("Can't set weapon target, no weapon in slot {weapon_index}.");
            return;
        };
        weapon.set_beam_target(target);
    }

    pub fn install_weapon(&mut self, index: usize, weapon: Weapon) {
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
        Ok(self.entries.remove(index).take())
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

#[derive(Debug)]
pub enum WeaponEntry {
    Projectile(WeaponStatus<ProjectileWeapon>),
    Beam(WeaponStatus<BeamWeapon>),
}

impl WeaponEntry {
    pub fn new(weapon: Weapon) -> Self {
        match weapon {
            Weapon::Projectile(weapon) => Self::Projectile(WeaponStatus {
                weapon,
                power_targeting: PowerTargetingStatus::Unpowered,
                charge: 0.0,
            }),
            Weapon::Beam(weapon) => Self::Beam(WeaponStatus {
                weapon,
                power_targeting: PowerTargetingStatus::Unpowered,
                charge: 0.0,
            }),
        }
    }

    pub fn weapon(&self) -> WeaponId {
        match self {
            WeaponEntry::Projectile(status) => WeaponId::Projectile(status.weapon.id()),
            WeaponEntry::Beam(status) => WeaponId::Beam(status.weapon.id()),
        }
    }

    pub fn is_powered(&self) -> bool {
        match self {
            WeaponEntry::Projectile(x) => x.is_powered(),
            WeaponEntry::Beam(x) => x.is_powered(),
        }
    }

    pub fn add_power(&mut self) {
        match self {
            WeaponEntry::Projectile(status) => {
                status.power_targeting = PowerTargetingStatus::Powered { target: None };
            }
            WeaponEntry::Beam(status) => {
                status.power_targeting = PowerTargetingStatus::Powered { target: None };
            }
        }
    }

    pub fn remove_power(&mut self) {
        match self {
            WeaponEntry::Projectile(status) => {
                status.power_targeting = PowerTargetingStatus::Unpowered;
            }
            WeaponEntry::Beam(status) => {
                status.power_targeting = PowerTargetingStatus::Unpowered;
            }
        }
    }

    pub fn charge_and_fire(&mut self, missiles: &mut usize, autofire: bool) -> Option<Volley> {
        match self {
            WeaponEntry::Projectile(status) => status
                .charge_and_fire(missiles, autofire)
                .map(Volley::Projectile),
            WeaponEntry::Beam(status) => {
                status.charge_and_fire(missiles, autofire).map(Volley::Beam)
            }
        }
    }

    pub fn set_room_target(&mut self, new_target: Option<RoomTarget>, targeting_self: bool) {
        let Self::Projectile(status) = self else {
            eprintln!("Can't set weapon target to room, weapon is not a projectile weapon.");
            return;
        };
        let PowerTargetingStatus::Powered { target } = &mut status.power_targeting else {
            eprintln!("Can't set weapon target, weapon is unpowered.");
            return;
        };
        if targeting_self && !status.weapon.id().can_target_self {
            eprintln!("Can't set weapon target, weapon cannot target self.");
            return;
        }
        *target = new_target;
    }

    pub fn set_beam_target(&mut self, new_target: Option<BeamTarget>) {
        let Self::Beam(status) = self else {
            eprintln!("Can't set weapon target, weapon is not a beam weapon.");
            return;
        };
        let PowerTargetingStatus::Powered { target } = &mut status.power_targeting else {
            eprintln!("Can't set weapon target, weapon is unpowered.");
            return;
        };
        *target = new_target;
    }

    pub fn target(&self) -> Option<WeaponTarget> {
        match self {
            WeaponEntry::Projectile(status) => status.target().map(WeaponTarget::Projectile),
            WeaponEntry::Beam(status) => status.target().map(WeaponTarget::Beam),
        }
    }

    pub fn charge(&self) -> f32 {
        match self {
            WeaponEntry::Projectile(status) => status.charge,
            WeaponEntry::Beam(status) => status.charge,
        }
    }

    pub fn take(self) -> Weapon {
        match self {
            WeaponEntry::Projectile(x) => Weapon::Projectile(x.weapon),
            WeaponEntry::Beam(x) => Weapon::Beam(x.weapon),
        }
    }
}

#[derive(Debug)]
pub struct WeaponStatus<Kind: Weaponlike + 'static> {
    /// The "physical" weapon. This can't be cloned. It can only be moved around and eventually
    /// destructed (tossed off into space).
    pub weapon: Kind,
    power_targeting: PowerTargetingStatus<Kind>,
    pub charge: f32,
}

impl<Kind: Weaponlike + 'static> WeaponStatus<Kind> {
    pub fn is_powered(&self) -> bool {
        matches!(self.power_targeting, PowerTargetingStatus::Powered { .. })
    }

    pub fn target(&self) -> Option<<Kind as Weaponlike>::Target> {
        match self.power_targeting {
            PowerTargetingStatus::Unpowered => None,
            PowerTargetingStatus::Powered { target } => target,
        }
    }

    #[must_use]
    pub fn charge_and_fire(
        &mut self,
        missiles: &mut usize,
        autofire: bool,
    ) -> Option<VolleyInner<Kind>> {
        let weapon = <Kind::Id as Into<WeaponId>>::into(self.weapon.id());
        if let PowerTargetingStatus::Powered { target } = &mut self.power_targeting {
            self.charge = (self.charge + 1.0 / 64.0).min(weapon.common().charge_time);
            if self.charge == weapon.common().charge_time {
                if let Some(target_room) = target.take() {
                    self.charge = 0.0;
                    if weapon.uses_missile() {
                        *missiles -= 1;
                    }
                    if autofire {
                        *target = Some(target_room);
                    }
                    return Some(VolleyInner {
                        weapon: self.weapon.id(),
                        target: target_room,
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
pub enum PowerTargetingStatus<Kind: Weaponlike> {
    Unpowered,
    Powered {
        target: Option<<Kind as Weaponlike>::Target>,
    },
}

pub enum Volley {
    Projectile(VolleyInner<ProjectileWeapon>),
    Beam(VolleyInner<BeamWeapon>),
}

pub struct VolleyInner<Kind: Weaponlike + 'static> {
    pub weapon: Kind::Id,
    pub target: Kind::Target,
}
