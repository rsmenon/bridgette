use std::fmt;
use std::str::FromStr;

use rand::Rng;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Seat {
    North,
    East,
    South,
    West,
}

impl Seat {
    pub const ALL: [Seat; 4] = [Seat::North, Seat::East, Seat::South, Seat::West];

    pub fn random() -> Seat {
        Seat::ALL[rand::thread_rng().gen_range(0..4)]
    }

    pub fn next(self) -> Seat {
        match self {
            Seat::North => Seat::East,
            Seat::East => Seat::South,
            Seat::South => Seat::West,
            Seat::West => Seat::North,
        }
    }

    pub fn partner(self) -> Seat {
        self.next().next()
    }

    pub fn index(self) -> usize {
        match self {
            Seat::North => 0,
            Seat::East => 1,
            Seat::South => 2,
            Seat::West => 3,
        }
    }

    pub fn short(self) -> &'static str {
        match self {
            Seat::North => "N",
            Seat::East => "E",
            Seat::South => "S",
            Seat::West => "W",
        }
    }

    pub fn is_ns(self) -> bool {
        matches!(self, Seat::North | Seat::South)
    }
}

impl fmt::Display for Seat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Seat::North => write!(f, "North"),
            Seat::East => write!(f, "East"),
            Seat::South => write!(f, "South"),
            Seat::West => write!(f, "West"),
        }
    }
}

impl FromStr for Seat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "North" | "N" => Ok(Seat::North),
            "East" | "E" => Ok(Seat::East),
            "South" | "S" => Ok(Seat::South),
            "West" | "W" => Ok(Seat::West),
            _ => Err(format!("invalid seat: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Phase {
    Bidding,
    Playing,
    Finished,
}
