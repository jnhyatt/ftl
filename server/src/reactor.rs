#[derive(Clone, Debug)]
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
