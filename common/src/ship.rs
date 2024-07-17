use bevy::{prelude::Component, reflect::Reflect};
use serde::{Deserialize, Serialize};
use strum_macros::EnumIter;

use crate::nav::Cell;

#[derive(Reflect, Serialize, Deserialize, EnumIter, Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Room {
    pub cells: Vec<Cell>,
}

impl Room {
    pub fn has_cell(&self, cell: Cell) -> bool {
        self.cells.iter().any(|x| *x == cell)
    }
}

#[derive(Component, Serialize, Deserialize, Debug, Default)]
pub struct Dead;
