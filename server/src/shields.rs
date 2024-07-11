use crate::{
    reactor::Reactor,
    ship_system::{PowerContext, ShipSystem, SystemStatus},
};

#[derive(Debug, Default, Clone)]
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

    fn add_power(&mut self, reactor: &mut Reactor, _context: PowerContext) {
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
