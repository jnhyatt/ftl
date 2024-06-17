use super::*;
use bevy::prelude::*;

#[derive(Component)]
pub struct PathGraph {
    /// Stores cell adjacency information. Must have an entry per cell, even
    /// if the association is empty. Each cell only contains
    /// adjacency for higher-numbered cells -- for example, the cell
    /// pair (3, 5) is stored as cells[3][5], not cells[5][3].
    cells: HashMap<usize, HashMap<usize, bool>>,
}

impl PathGraph {
    pub fn new(cell_count: usize) -> Self {
        Self {
            cells: (0..cell_count).map(|x| (x, [].into())).collect(),
        }
    }

    pub fn with_edge(mut self, from: usize, to: usize, door: bool) -> Self {
        self.cells.get_mut(&from).unwrap().insert(to, door);
        self.cells.get_mut(&to).unwrap().insert(from, door);
        self
    }

    pub fn find_path(&self, from: usize, to: usize) -> Option<Path> {
        let mut frontier = VecDeque::new();
        frontier.push_back(from);
        let mut came_from = HashMap::new();
        while let Some(current) = frontier.pop_front() {
            if current == to {
                break;
            }
            for &next in self.cells.get(&current).unwrap().keys() {
                if !came_from.contains_key(&next) {
                    frontier.push_back(next);
                    came_from.insert(next, current);
                }
            }
        }
        let mut path = Vec::new();
        let mut current = to;
        while current != from {
            path.push(current);
            current = *came_from.get(&current)?;
        }
        path.push(current);
        Some(Path(path))
    }

    pub fn all_cells(&self) -> impl Iterator<Item = usize> + Clone {
        0..self.cells.len()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NextPath {
    Next(Path),
    Complete(usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Path(Vec<usize>);

impl Path {
    pub fn start_at_next(mut self) -> NextPath {
        self.0.truncate(self.0.len() - 1);
        if let &[dest] = &self.0[..] {
            NextPath::Complete(dest)
        } else {
            NextPath::Next(self)
        }
    }

    pub fn goal(&self) -> usize {
        self.0[0]
    }

    pub fn current_leg(&self) -> (usize, usize) {
        let len = self.0.len();
        (self.0[len - 1], self.0[len - 2])
    }
}

#[derive(Component, Debug, Clone, PartialEq)]
pub enum Whereabouts {
    At(usize),
    /// The path this crew is taking to a goal, along with a progress along
    /// the current leg.
    _Moving(Path, f32),
}

impl Whereabouts {
    pub fn current_cell(&self) -> usize {
        match self {
            Whereabouts::At(x) => *x,
            Whereabouts::_Moving(path, t) if *t < 0.5 => path.current_leg().0,
            Whereabouts::_Moving(path, _) => path.current_leg().1,
        }
    }

    pub fn occupied_cell(&self) -> usize {
        match self {
            Whereabouts::At(x) => *x,
            Whereabouts::_Moving(path, _) => path.goal(),
        }
    }

    pub fn stepped(self) -> Self {
        match self {
            Whereabouts::At(x) => Whereabouts::At(x),
            Whereabouts::_Moving(path, t) if t == 1.0 => match path.start_at_next() {
                NextPath::Next(path) => Whereabouts::_Moving(path, 0.0),
                NextPath::Complete(dest) => Whereabouts::At(dest),
            },
            Whereabouts::_Moving(path, t) => Whereabouts::_Moving(path, (t + 1.0 / 32.0).min(1.0)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{NextPath, Path, PathGraph, Whereabouts};

    #[test]
    fn test_whereabouts() {
        let whereabouts = Whereabouts::At(3);
        assert_eq!(whereabouts.current_cell(), 3);
        assert_eq!(whereabouts.occupied_cell(), 3);
        assert_eq!(whereabouts.stepped(), Whereabouts::At(3));

        let whereabouts = Whereabouts::_Moving(Path([5, 4, 1, 0].into()), 0.9);
        assert_eq!(whereabouts.current_cell(), 1);
        assert_eq!(whereabouts.occupied_cell(), 5);
        assert_eq!(
            whereabouts.clone().stepped(),
            Whereabouts::_Moving(Path([5, 4, 1, 0].into()), 0.95)
        );
        assert_eq!(
            whereabouts.stepped().stepped(),
            Whereabouts::_Moving(Path([5, 4, 1].into()), 0.0)
        );
    }

    #[test]
    fn test_path() {
        let path = Path([3, 5, 2, 6, 10].into());
        assert_eq!(path.goal(), 3);
        assert_eq!(path.current_leg(), (10, 6));
        let NextPath::Next(path) = path.start_at_next() else {
            panic!("Expected next path");
        };
        assert_eq!(path, Path([3, 5, 2, 6].into()));
        let NextPath::Next(path) = path.start_at_next() else {
            panic!("Expected next path");
        };
        assert_eq!(path, Path([3, 5, 2].into()));
        let NextPath::Next(path) = path.start_at_next() else {
            panic!("Expected next path");
        };
        assert_eq!(path, Path([3, 5].into()));
        assert_eq!(path.start_at_next(), NextPath::Complete(3));
    }

    #[test]
    fn test_path_graph() {
        let graph = PathGraph::new(6)
            .with_edge(0, 1, true)
            .with_edge(1, 2, false)
            .with_edge(1, 3, false)
            .with_edge(1, 4, false)
            .with_edge(2, 3, false)
            .with_edge(2, 4, false)
            .with_edge(3, 4, false)
            .with_edge(4, 5, true);
        assert_eq!(graph.find_path(0, 5), Some(Path([5, 4, 1, 0].into())));
    }
}
