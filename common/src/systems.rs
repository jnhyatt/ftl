use super::*;

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct SystemStatus {
    upgrade_level: usize,
    damage: usize,
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
        }
    }
}

pub trait ShipSystem {
    fn system_status(&self) -> SystemStatus;
    fn system_status_mut(&mut self) -> &mut SystemStatus;
    fn current_power(&self) -> usize;
    fn add_power(&mut self, reactor: &mut Reactor);
    fn remove_power(&mut self, reactor: &mut Reactor);

    fn damage_system(&mut self, amount: usize, reactor: &mut Reactor) {
        let SystemStatus {
            upgrade_level,
            damage,
        } = self.system_status_mut();
        // Canonical impl for inevitable trait refactor
        // Cap max damage to our upgrade level
        let actual_amount = amount.min(*upgrade_level - *damage);
        // Apply damage
        *damage += actual_amount;
        // Compute new max power
        let new_max = *upgrade_level - *damage;
        // Reduce power until we're back within our system power budget
        while self.current_power() > new_max {
            self.remove_power(reactor);
        }
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
