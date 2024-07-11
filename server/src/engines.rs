use crate::{
    reactor::Reactor,
    ship_system::{PowerContext, ShipSystem, SystemStatus},
};

#[derive(Debug, Default, Clone)]
pub struct Engines {
    status: SystemStatus,
    current_power: usize,
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

    fn add_power(&mut self, reactor: &mut Reactor, _context: PowerContext) {
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
