use crate::context::Coord;

use std::fmt::{Display, Error, Formatter};

#[derive(Clone, Copy, Debug, PartialEq)]
#[must_use]
pub enum Action {
    Describe,
    Go,
    Wait,
    Quit,
}

impl Display for Action {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        write!(
            f,
            "{}",
            match self {
                Self::Describe => "describe",
                Self::Go => "go",
                Self::Wait => "wait",
                Self::Quit => "quit",
            }
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[must_use]
pub enum Direction {
    North,
    South,
    East,
    West,
}

impl Direction {
    pub const fn as_coord_with_magnitude(&self, m: i8) -> Coord {
        let (n, w) = match self {
            Self::North => (m, 0),
            Self::South => (-m, 0),
            Self::East => (0, -m),
            Self::West => (0, m),
        };
        Coord { n, w }
    }
}

impl Display for Direction {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        write!(
            f,
            "{}",
            match self {
                Self::North => "north",
                Self::South => "south",
                Self::East => "east",
                Self::West => "west",
            }
        )
    }
}
