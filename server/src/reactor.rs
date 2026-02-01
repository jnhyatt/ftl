#[derive(Clone, Debug)]
pub struct Reactor {
    pub upgrade_level: usize,
    pub available: usize,
}

impl Reactor {
    pub fn new() -> Self {
        Self {
            upgrade_level: 0,
            available: 0,
        }
    }

    pub fn upgrade(&mut self) {
        self.upgrade_level += 1;
        self.available += 1;
    }
}
