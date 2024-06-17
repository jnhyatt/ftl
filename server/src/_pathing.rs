use std::collections::{HashMap, VecDeque};

#[derive(Clone, Copy)]
pub struct DoorIndex;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Path(Vec<usize>);

pub struct PathGraph {
    pub cells: HashMap<usize, HashMap<usize, Option<DoorIndex>>>,
}

impl PathGraph {
    pub fn neighbors_of(&self, cell: usize) -> impl Iterator<Item = usize> + '_ {
        self.cells.get(&cell).unwrap().keys().cloned()
    }

    pub fn find_path(&self, from: usize, to: usize) -> Option<Path> {
        let mut frontier = VecDeque::new();
        frontier.push_back(from);
        let mut came_from = HashMap::new();
        while let Some(current) = frontier.pop_front() {
            if current == to {
                break;
            }
            for next in self.neighbors_of(current) {
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
}
