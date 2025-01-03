use common::ship::SystemId;

use crate::{
    reactor::Reactor,
    ship_system::{boring_add_power, boring_remove_power, PowerContext, ShipSystem, SystemStatus},
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
        boring_add_power(
            self.status.max_power(),
            &mut self.current_power,
            reactor,
            SystemId::Engines,
        );
    }

    fn remove_power(&mut self, reactor: &mut Reactor) {
        boring_remove_power(&mut self.current_power, reactor, SystemId::Engines);
    }
}
