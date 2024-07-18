use bevy::{math::Vec2, prelude::Component, reflect::Reflect};
use serde::{Deserialize, Serialize};
use strum_macros::EnumIter;

use crate::{
    nav::{Cell, LineSection, SquareSection},
    util::IterAvg,
};

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

#[derive(Debug)]
pub struct Room {
    pub cells: &'static [Cell],
}

impl Room {
    pub fn has_cell(&self, cell: Cell) -> bool {
        self.cells.iter().any(|x| *x == cell)
    }
}

#[derive(Component, Serialize, Deserialize, Debug, Default)]
pub struct Dead;

#[derive(Component, Debug)]
pub struct ShipType {
    pub rooms: &'static [Room],
    pub nav_mesh: (&'static [LineSection], &'static [SquareSection]),
    pub path_graph: &'static [(Cell, &'static [Cell])],
    pub cell_positions: &'static [Vec2],
}

impl ShipType {
    pub fn room_center(&self, room: usize) -> Vec2 {
        self.rooms[room]
            .cells
            .iter()
            .map(|&Cell(x)| self.cell_positions[x])
            .average()
            .unwrap()
    }
}

pub const SHIPS: [ShipType; 1] = [ShipType {
    rooms: &[
        Room {
            cells: &[Cell(0), Cell(1), Cell(2), Cell(3)],
        },
        Room {
            cells: &[Cell(4), Cell(5)],
        },
        Room {
            cells: &[Cell(6), Cell(7)],
        },
    ],
    nav_mesh: (
        &[
            LineSection([Cell(4), Cell(5)]),
            LineSection([Cell(6), Cell(7)]),
            LineSection([Cell(3), Cell(5)]),
            LineSection([Cell(5), Cell(7)]),
        ],
        &[SquareSection([[Cell(0), Cell(1)], [Cell(2), Cell(3)]])],
    ),
    path_graph: &[
        (Cell(0), &[Cell(1), Cell(2), Cell(3)]),
        (Cell(1), &[Cell(0), Cell(2), Cell(3)]),
        (Cell(2), &[Cell(0), Cell(1), Cell(3)]),
        (Cell(3), &[Cell(0), Cell(1), Cell(2), Cell(5)]),
        (Cell(4), &[Cell(5)]),
        (Cell(5), &[Cell(3), Cell(4), Cell(7)]),
        (Cell(6), &[Cell(7)]),
        (Cell(7), &[Cell(5), Cell(6)]),
    ],
    cell_positions: &[
        Vec2::new(-1.5, -0.5),
        Vec2::new(-0.5, -0.5),
        Vec2::new(-1.5, 0.5),
        Vec2::new(-0.5, 0.5),
        Vec2::new(0.5, -0.5),
        Vec2::new(0.5, 0.5),
        Vec2::new(1.5, -0.5),
        Vec2::new(1.5, 0.5),
    ],
}];
