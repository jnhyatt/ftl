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
            cells: &[Cell(0), Cell(1)],
        },
        Room {
            cells: &[Cell(2), Cell(3), Cell(4), Cell(5)],
        },
        Room {
            cells: &[Cell(6), Cell(7), Cell(8), Cell(9)],
        },
        Room {
            cells: &[Cell(10), Cell(11), Cell(12), Cell(13)],
        },
        Room {
            cells: &[Cell(14), Cell(15)],
        },
        Room {
            cells: &[Cell(16), Cell(17)],
        },
    ],
    nav_mesh: (
        &[
            LineSection([Cell(0), Cell(1)]),
            LineSection([Cell(1), Cell(6)]),
            LineSection([Cell(5), Cell(8)]),
            LineSection([Cell(8), Cell(17)]),
            LineSection([Cell(9), Cell(12)]),
            LineSection([Cell(13), Cell(15)]),
            LineSection([Cell(14), Cell(15)]),
            LineSection([Cell(16), Cell(17)]),
        ],
        &[
            SquareSection([[Cell(2), Cell(3)], [Cell(4), Cell(5)]]),
            SquareSection([[Cell(6), Cell(7)], [Cell(8), Cell(9)]]),
            SquareSection([[Cell(10), Cell(11)], [Cell(12), Cell(13)]]),
        ],
    ),
    path_graph: &[
        (Cell(0), &[Cell(1)]),
        (Cell(1), &[Cell(0), Cell(6)]),
        (Cell(2), &[Cell(3), Cell(4), Cell(5)]),
        (Cell(3), &[Cell(2), Cell(4), Cell(5)]),
        (Cell(4), &[Cell(2), Cell(3), Cell(5)]),
        (Cell(5), &[Cell(2), Cell(3), Cell(4), Cell(8)]),
        (Cell(6), &[Cell(1), Cell(7), Cell(8), Cell(9)]),
        (Cell(7), &[Cell(6), Cell(8), Cell(9)]),
        (Cell(8), &[Cell(5), Cell(6), Cell(7), Cell(9), Cell(17)]),
        (Cell(9), &[Cell(6), Cell(7), Cell(8), Cell(12)]),
        (Cell(10), &[Cell(11), Cell(12), Cell(13)]),
        (Cell(11), &[Cell(10), Cell(12), Cell(13)]),
        (Cell(12), &[Cell(9), Cell(10), Cell(11), Cell(13)]),
        (Cell(13), &[Cell(10), Cell(11), Cell(12), Cell(15)]),
        (Cell(14), &[Cell(15)]),
        (Cell(15), &[Cell(13), Cell(14)]),
        (Cell(16), &[Cell(17)]),
        (Cell(17), &[Cell(8), Cell(16)]),
    ],
    cell_positions: &[
        Vec2::new(-2.0, -1.5),
        Vec2::new(-1.0, -1.5),
        Vec2::new(-3.0, -0.5),
        Vec2::new(-2.0, -0.5),
        Vec2::new(-3.0, 0.5),
        Vec2::new(-2.0, 0.5),
        Vec2::new(-1.0, -0.5),
        Vec2::new(0.0, -0.5),
        Vec2::new(-1.0, 0.5),
        Vec2::new(0.0, 0.5),
        Vec2::new(1.0, -0.5),
        Vec2::new(2.0, -0.5),
        Vec2::new(1.0, 0.5),
        Vec2::new(2.0, 0.5),
        Vec2::new(3.0, -0.5),
        Vec2::new(3.0, 0.5),
        Vec2::new(-2.0, 1.5),
        Vec2::new(-1.0, 1.5),
    ],
}];
