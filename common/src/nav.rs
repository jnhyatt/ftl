use crate::util::{round_to_usize, MoveToward};
use bevy::math::Vec2;
use nonempty_collections::{IntoNonEmptyIterator, NEVec, NonEmptyIterator};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    num::NonZeroUsize,
    task::Poll,
};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Cell(pub usize);

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum CrewNavStatus {
    At(Cell),
    Navigating(CrewNav),
}

impl CrewNavStatus {
    pub fn step(&mut self, nav_mesh: &NavMesh) {
        // Only need to update if we're navigating
        let Self::Navigating(nav) = self else {
            return;
        };
        if let Poll::Ready(destination) = nav.step(nav_mesh) {
            *self = Self::At(destination);
        }
    }

    /// This crew's current goal cell. Only one crew can occupy a cell at a time. This crew may or
    /// may not be anywhere near this cell. This is how we enforce only one crew per cell.
    pub fn occupied_cell(&self) -> Cell {
        match self {
            CrewNavStatus::At(x) => *x,
            CrewNavStatus::Navigating(nav) => nav.path.goal(),
        }
    }

    /// This crew's current location, which can be either at a cell or currently traversing a nav
    /// section. This represents the crew's physical position rather than their goal. This is used
    /// as the starting point for pathfinding.
    pub fn current_location(&self) -> CrewLocation {
        match self {
            &CrewNavStatus::At(cell) => CrewLocation::Cell(cell),
            CrewNavStatus::Navigating(nav) => CrewLocation::NavSection(nav.nav_section()),
        }
    }

    /// The cell this crew is closest to. This is used mostly for violence... crew lose health when
    /// they're in a room on fire or exposed to vacuum, or when their room gets hit by a projectile
    /// or beam. Also crew determine attack targets based on their current room.
    pub fn current_cell(&self) -> Cell {
        match self {
            CrewNavStatus::At(cell) => *cell,
            CrewNavStatus::Navigating(nav) => nav.current_cell(),
        }
    }
}

/// The navigation state of a crew member. Contains their current path and location on the nav mesh.
/// [`CrewNav::current_location`] is the crew's instantaneous position on the nav mesh -- for
/// example, "between cell 3 and cell 5, 25% of the way there" -- while [`CrewNav::path`] is the
/// sequence of cells through which they will navigate to reach their goal.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CrewNav {
    pub path: Path,
    pub current_location: NavLocation,
}

impl CrewNav {
    /// Advance by one fixed update step. This will move the crew along its current [`NavSection`]
    /// and update its progress along its [`Path`] if it's made it all the way across it. If the
    /// crew has reached the end of the path, this will return [`Poll::Ready`] with the [`Cell`]
    /// that was reached, or [`Poll::Pending`] otherwise. This is maybe an abuse of [`Poll`], but in
    /// my head, it's semantically more clear than returning an `Option<Cell>`.
    fn step(&mut self, nav_mesh: &NavMesh) -> Poll<Cell> {
        let current_goal = self.path.next_waypoint();
        // Get target coordinate within nav section and step ourselves toward it
        // TODO move this logic to `NavLocation`
        let arrived = match &mut self.current_location {
            NavLocation::Line(line, x) => {
                let target_x = line.coords_of(current_goal);
                *x = x.move_toward(target_x, 1.0 / 36.0);
                *x == target_x
            }
            NavLocation::Square(square, x) => {
                let target_x = square.coords_of(current_goal);
                *x = x.move_toward(target_x, 1.0 / 36.0);
                *x == target_x
            }
        };
        // If we've arrived, update our current location to the next nav section in our path
        if arrived {
            let Some(next_path) = self.path.clone().step() else {
                return Poll::Ready(current_goal);
            };
            self.path = next_path;
            let next_section = nav_mesh
                .section_with_cells(current_goal, self.path.next_waypoint())
                .unwrap();
            self.current_location = next_section.to_location(current_goal);
        }
        Poll::Pending
    }

    /// The [`NavSection`] this crew is currently traversing. Used for determining starting point
    /// for pathfinding.
    fn nav_section(&self) -> NavSection {
        match self.current_location {
            NavLocation::Line(x, _) => NavSection::Line(x),
            NavLocation::Square(x, _) => NavSection::Square(x),
        }
    }

    /// The cell this crew is currently closest to. Used for violence, see
    /// [`CrewNavStatus::current_cell`].
    fn current_cell(&self) -> Cell {
        match self.current_location {
            NavLocation::Line(line, x) => line.0[round_to_usize(x)],
            NavLocation::Square(square, x) => square.0[round_to_usize(x.y)][round_to_usize(x.x)],
        }
    }
}

/// A crew member's current location on the nav mesh, either at a cell or traversing a nav section.
/// Used to determine starting point for pathfinding.
pub enum CrewLocation {
    Cell(Cell),
    NavSection(NavSection),
}

/// Holds information about how a ship's cells are connected for pathfinding purposes. 2x2 rooms are
/// represented as squares, while 1x2 rooms and paths through doors are represented as lines. The
/// only difference is that squares can be traversed diagonally.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct NavMesh {
    pub lines: Vec<LineSection>,
    pub squares: Vec<SquareSection>,
}

impl NavMesh {
    pub fn sections(&self) -> impl Iterator<Item = NavSection> + '_ {
        let lines = self.lines.iter().cloned().map(NavSection::Line);
        let squares = self.squares.iter().cloned().map(NavSection::Square);
        lines.chain(squares)
    }

    /// Find the [`NavSection`] that contains the path between `a` and `b`. If the graph was
    /// constructed correctly, there will be at most one.
    pub fn section_with_cells(&self, a: Cell, b: Cell) -> Option<NavSection> {
        self.sections().find(|x| x.contains(a) && x.contains(b))
    }

    /// Find the shortest path from `start` to the goal represented in `pathing`, or `None` if the
    /// goal is unreachable from the given start position (or if the crew is already at the goal).
    pub fn find_path(&self, pathing: &GoalPathing, start: CrewLocation) -> Option<Path> {
        let cost_to_goal = |mut cell: Cell| {
            let mut cost = 0usize;
            while let Some(next) = pathing.came_from.get(&cell) {
                cell = *next;
                cost += 1;
            }
            return cost;
        };
        let start = match start {
            // If we start in a cell, our next waypoint is just `came_from[start]`
            CrewLocation::Cell(cell) => pathing.came_from.get(&cell).cloned(),
            // If we start in a nav section, our next waypoint is the cell in that section with the lowest cost-to-goal
            CrewLocation::NavSection(section) => section.cells().min_by_key(|x| cost_to_goal(*x)),
        };
        let Some(start) = start else {
            return None;
        };
        let mut path = NEVec::new(start);
        while let Some(&next) = pathing.came_from.get(path.last()) {
            path.push(next);
        }
        path.reverse();
        Some(Path(path))
    }
}

/// A [`NavMesh`] section. Can either be a line spanning two cells or a square with a cell at each
/// corner. A crew member on this section must have coordinates clamped to [0, 1] for each
/// dimension. For a line, the cells are at `x=0` and `x=1`. For a square, the cells are at
/// coordinates where x and y are both either 0 or 1. If a crew member is at one of these points,
/// they are considered to be on the cell indicated by this section. In that case, the crew member
/// can also be considered to be on *any* nav section that contains that cell. Crew traverse the nav
/// mesh by moving their coordinates along a nav section until they are at a shared cell, then
/// moving to the same cell in a different nav section, repeating until they arrive at their
/// destination.
#[derive(Clone, Copy, Debug)]
pub enum NavSection {
    Line(LineSection),
    Square(SquareSection),
}

impl NavSection {
    /// Creates a [`NavLocation`] on this section with the coordinates corresponding to `cell`.
    pub fn to_location(self, cell: Cell) -> NavLocation {
        match self {
            NavSection::Line(x) => NavLocation::Line(x, x.coords_of(cell)),
            NavSection::Square(x) => NavLocation::Square(x, x.coords_of(cell)),
        }
    }

    /// Whether this nav section extends to `cell`.
    pub fn contains(&self, cell: Cell) -> bool {
        self.cells().any(|x| x == cell)
    }

    pub fn cells(&self) -> impl Iterator<Item = Cell> {
        match self {
            &NavSection::Line(LineSection([a, b])) => vec![a, b].into_iter(),
            &NavSection::Square(SquareSection([[a, b], [c, d]])) => vec![a, b, c, d].into_iter(),
        }
        .into_iter()
    }
}

/// A [`NavMesh`] section with one dimension. A crew member on this section should have a single
/// coordinate in [0, 1]. 0 and 1 correspond to `self.0[0]` and `self.0[1]`, respectively.
#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub struct LineSection(pub [Cell; 2]);

impl LineSection {
    /// Return the coordinate of the given cell within this nav mesh section. Panics if `cell` is
    /// not in the section.
    pub fn coords_of(&self, cell: Cell) -> f32 {
        self.0
            .iter()
            .position(|x| *x == cell)
            .map(|x| x as f32)
            .unwrap()
    }
}

/// A [`NavMesh`] section with two dimensions.
#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub struct SquareSection(pub [[Cell; 2]; 2]);

impl SquareSection {
    /// Return the coordinate of the given cell within this nav mesh section. Panics if `cell` is
    /// not in the section.
    pub fn coords_of(&self, cell: Cell) -> Vec2 {
        self.0
            .iter()
            .enumerate()
            .find_map(|(i, row)| row.iter().position(|x| *x == cell).map(|j| (i, j)))
            .map(|(i, j)| Vec2::new(i as f32, j as f32))
            .unwrap()
    }
}

/// This is a crew member's instantaneous location on the [`NavMesh`]. It's essentially a union of
/// two `enum`s: [`NavSection`] and a 1D/2D coordinate. It could be a `struct` with two fields:
/// ```
/// pub struct NavLocation {
///     pub section: NavSection,
///     pub coordinate: NavCoord,
/// }
/// ```
/// The problem with this design is that now you have to keep two `enum`s in sync. The tradeoff is
/// you either duplicate type definitions (this design) or have unenforced invariants (two-field
/// structure). I opted for this design because it means I have fewer `unreachable!` `match` arms,
/// but conceptually, it might help to think of it as the above `struct`.
#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub enum NavLocation {
    Line(LineSection, f32),
    Square(SquareSection, Vec2),
}

/// Responsible for generating a [`Path`]. The [`PathGraph`] is generated from a `Cells`, a
/// [`NavMesh`], and an initial [`NavLocation`]. Stores a set of edges for each [`Cell`].
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PathGraph {
    // TODO make private
    pub edges: HashMap<Cell, HashSet<Cell>>,
}

impl PathGraph {
    pub fn neighbors_of(&self, cell: Cell) -> impl Iterator<Item = Cell> + '_ {
        self.edges.get(&cell).unwrap().iter().cloned()
    }

    /// Generate a [`GoalPathing`] for reaching `goal` from any other cell.
    pub fn pathing_to(&self, goal: Cell) -> GoalPathing {
        let mut frontier = VecDeque::new();
        frontier.push_back(goal);
        let mut came_from = HashMap::new();
        while let Some(current) = frontier.pop_front() {
            for next in self.neighbors_of(current) {
                if next == goal {
                    continue;
                }
                if !came_from.contains_key(&next) {
                    frontier.push_back(next);
                    came_from.insert(next, current);
                }
            }
        }
        GoalPathing { came_from }
    }
}

/// Information on how to get to a goal cell from any other cell. [`GoalPathing::came_from`] maps
/// each cell to the next cell on the path toward the goal. If a cell is not in the map, it is
/// either the goal cell itself or unreachable from the goal. In either case, a crew wanting to
/// travel to such a cell will not be given a path.
#[derive(Debug, Clone)]
pub struct GoalPathing {
    came_from: HashMap<Cell, Cell>,
}

/// Represents a sequence of waypoints to get from the current cell to a target cell.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Path(NEVec<Cell>);

impl Path {
    pub fn goal(&self) -> Cell {
        *self.0.first()
    }

    /// Returns the next [`Cell`] in the path, or [`None`] if the path is empty. An empty path
    /// indicates path completion.
    pub fn next_waypoint(&self) -> Cell {
        *self.0.last()
    }

    pub fn step(self) -> Option<Self> {
        let new_len = NonZeroUsize::new(self.0.len().get() - 1)?;
        Some(Path(self.0.into_nonempty_iter().take(new_len).collect()))
    }
}

#[cfg(test)]
mod tests {
    use nonempty_collections::nev;

    use super::*;

    fn nav_mesh() -> NavMesh {
        NavMesh {
            lines: vec![
                LineSection([Cell(4), Cell(5)]),
                LineSection([Cell(6), Cell(7)]),
                LineSection([Cell(3), Cell(5)]),
                LineSection([Cell(5), Cell(7)]),
                LineSection([Cell(8), Cell(9)]),
            ],
            squares: vec![SquareSection([[Cell(0), Cell(1)], [Cell(2), Cell(3)]])],
        }
    }

    fn path_graph() -> PathGraph {
        PathGraph {
            edges: [
                (Cell(0), [Cell(1), Cell(2), Cell(3)].into()),
                (Cell(1), [Cell(0), Cell(2), Cell(3)].into()),
                (Cell(2), [Cell(0), Cell(1), Cell(3)].into()),
                (Cell(3), [Cell(0), Cell(1), Cell(2), Cell(5)].into()),
                (Cell(4), [Cell(5)].into()),
                (Cell(5), [Cell(3), Cell(4), Cell(7)].into()),
                (Cell(6), [Cell(7)].into()),
                (Cell(7), [Cell(5), Cell(6)].into()),
                (Cell(8), [Cell(9)].into()),
                (Cell(9), [Cell(8)].into()),
            ]
            .into(),
        }
    }

    #[test]
    fn line_coords_of() {
        let line = LineSection([Cell(3), Cell(7)]);
        assert_eq!(line.coords_of(Cell(3)), 0.0);
        assert_eq!(line.coords_of(Cell(7)), 1.0);
    }

    #[test]
    fn nav_a_to_b() {
        let nav_mesh = nav_mesh();
        let path = Path(nev![Cell(6), Cell(7), Cell(5), Cell(3)]);
        let current_location = NavLocation::Square(
            SquareSection([[Cell(0), Cell(1)], [Cell(2), Cell(3)]]),
            Vec2::new(0.0, 0.0),
        );
        let mut crew = CrewNavStatus::Navigating(CrewNav {
            path,
            current_location,
        });
        loop {
            match crew {
                CrewNavStatus::At(x) => {
                    assert_eq!(x, Cell(6));
                    break;
                }
                _ => {
                    crew.step(&nav_mesh);
                }
            }
        }
    }

    #[test]
    fn nav_b_to_a() {
        let nav_mesh = nav_mesh();
        let path = Path(nev![Cell(6), Cell(7), Cell(5), Cell(3)]);
        let current_location = NavLocation::Square(
            SquareSection([[Cell(0), Cell(1)], [Cell(2), Cell(3)]]),
            Vec2::new(0.0, 0.0),
        );
        let mut crew = CrewNavStatus::Navigating(CrewNav {
            path,
            current_location,
        });
        loop {
            match crew {
                CrewNavStatus::At(x) => {
                    assert_eq!(x, Cell(6));
                    break;
                }
                _ => {
                    crew.step(&nav_mesh);
                }
            }
        }
    }

    #[test]
    fn path_to() {
        let nav_mesh = nav_mesh();
        let path_graph = path_graph();
        let pathing = path_graph.pathing_to(Cell(6));

        let path = nav_mesh.find_path(&pathing, CrewLocation::Cell(Cell(0)));
        assert_eq!(path, Some(Path(nev![Cell(6), Cell(7), Cell(5), Cell(3)])));
        let path = nav_mesh.find_path(
            &pathing,
            CrewLocation::NavSection(NavSection::Square(nav_mesh.squares[0])),
        );
        assert_eq!(path, Some(Path(nev![Cell(6), Cell(7), Cell(5), Cell(3)])));
        let path = nav_mesh.find_path(&pathing, CrewLocation::Cell(Cell(8)));
        assert_eq!(path, None);
        let path = nav_mesh.find_path(&pathing, CrewLocation::Cell(Cell(6)));
        assert_eq!(path, None);
    }
}
