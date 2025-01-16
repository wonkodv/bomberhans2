use core::fmt;

use serde::Deserialize;
use serde::Serialize;

pub fn random(time: GameTime, r1: i32, r2: i32) -> u32 {
    // TODO:  test / improve randomness
    let mut x: u32 = 42;
    for i in [time.ticks_from_start(), r1 as u32, r2 as u32] {
        for b in i.to_le_bytes() {
            x = x.overflowing_add(b.into()).0.overflowing_mul(31).0;
        }
    }
    x
}

pub trait Idx {
    fn idx(self) -> usize;
}

impl<T> Idx for T
where
    usize: TryFrom<T>,
    <usize as TryFrom<T>>::Error: std::fmt::Debug,
{
    fn idx(self) -> usize {
        let r: Result<usize, <usize as TryFrom<T>>::Error> = usize::try_from(self);
        r.expect("Index can be converted to usize")
    }
}

pub const TICKS_PER_SECOND: u32 = 50;
pub const TIME_PER_TICK: std::time::Duration = std::time::Duration::from_millis(20);

/// A Time Stamp (not a duration)
#[derive(Default, Copy, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct GameTime {
    inner: u32,
}

impl GameTime {
    pub fn new() -> Self {
        Self { inner: 0 }
    }
    pub fn ticks_from_start(self) -> u32 {
        self.inner
    }
}

impl fmt::Debug for GameTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "⌚{}", self.inner)
    }
}

impl std::ops::Add<GameTimeDiff> for GameTime {
    type Output = Self;

    fn add(self, rhs: GameTimeDiff) -> Self::Output {
        Self {
            inner: self.inner + rhs.ticks,
        }
    }
}

/// A Duration
#[derive(Copy, Clone, PartialEq, PartialOrd)]
pub struct GameTimeDiff {
    ticks: u32,
}

impl GameTimeDiff {
    pub fn from_ticks(ticks: u32) -> Self {
        Self { ticks }
    }

    pub fn from_ms(milliseconds: u32) -> Self {
        let ticks = if milliseconds == 0 {
            0
        } else {
            u32::max(1, (milliseconds * TICKS_PER_SECOND + 499) / 1000)
        };
        Self { ticks }
    }
    pub fn ticks(self) -> u32 {
        self.ticks
    }
}

impl fmt::Debug for GameTimeDiff {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "⏳{}", self.ticks)
    }
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PlayerId(pub u32);

impl fmt::Debug for PlayerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Player{}", self.0)
    }
}

impl Idx for PlayerId {
    fn idx(self) -> usize {
        self.0.try_into().unwrap()
    }
}

#[derive(Copy, Clone, PartialEq, Serialize, Deserialize)]
pub enum Direction {
    North,
    West,
    South,
    East,
}

impl Direction {
    pub fn left(self) -> Self {
        match self {
            Direction::North => Direction::West,
            Direction::West => Direction::South,
            Direction::South => Direction::East,
            Direction::East => Direction::North,
        }
    }
    pub fn right(self) -> Self {
        match self {
            Direction::North => Direction::East,
            Direction::West => Direction::North,
            Direction::South => Direction::West,
            Direction::East => Direction::South,
        }
    }
}

impl fmt::Debug for Direction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Direction::North => write!(f, "⬆️"),
            Direction::West => write!(f, "⬅️"),
            Direction::South => write!(f, "⬇️"),
            Direction::East => write!(f, "➡️"),
        }
    }
}

/// Index of a cell
#[derive(Copy, Clone, PartialEq)]
pub struct CellPosition {
    pub x: i32,
    pub y: i32,
}

impl CellPosition {
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    /// move position `distance` into  `direction`
    pub fn add(self, direction: Direction, distance: i32) -> Self {
        let Self { x, y } = self;
        let (x, y) = match direction {
            Direction::North => (x, y - distance),
            Direction::West => (x - distance, y),
            Direction::South => (x, y + distance),
            Direction::East => (x + distance, y),
        };
        Self::new(x, y)
    }
}

impl fmt::Debug for CellPosition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({:02}/{:02})", self.x, self.y)
    }
}

/// Player positions
#[derive(Copy, Clone, PartialEq, Serialize, Deserialize)]
pub struct Position {
    pub x: i32,
    pub y: i32,
}

impl fmt::Debug for Position {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{:02}/{:02}]", self.x, self.y)
    }
}

impl Position {
    /// Player position is tracked in this many fractions of a cell
    pub const ACCURACY: i32 = 100;

    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    /// move position `distance` into  `direction`
    pub fn add(self, direction: Direction, distance: i32) -> Self {
        let Self { x, y } = self;
        let (x, y) = match direction {
            Direction::North => (x, y - distance),
            Direction::West => (x - distance, y),
            Direction::South => (x, y + distance),
            Direction::East => (x + distance, y),
        };
        Self::new(x, y)
    }

    pub fn as_cell_pos(self) -> CellPosition {
        CellPosition {
            x: self.x / Self::ACCURACY,
            y: self.y / Self::ACCURACY,
        }
    }

    pub fn from_cell_position(p: CellPosition) -> Self {
        Self {
            x: p.x * Self::ACCURACY + Self::ACCURACY / 2,
            y: p.y * Self::ACCURACY + Self::ACCURACY / 2,
        }
    }

    pub fn distance_to_border(self, direction: Direction) -> i32 {
        match direction {
            Direction::North => self.y % Position::ACCURACY,
            Direction::South => 100 - self.y % Position::ACCURACY,
            Direction::West => self.x % Position::ACCURACY,
            Direction::East => 100 - self.x % Position::ACCURACY,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_random() {
        let r = random(GameTime::default(), 0, 0);
        assert_eq!(r, random(GameTime::default(), 0, 0));
        assert!(r != random(GameTime::default() + GameTimeDiff::from_ticks(1), 0, 0));
        assert!(r != random(GameTime::default(), 1, 0));
        assert!(r != random(GameTime::default(), 0, 1));
    }

    #[test]
    fn test_player_coord_add() {
        let p = Position { x: 100, y: 100 };

        assert_eq!(p.add(Direction::North, 101), Position { x: 100, y: -1 });

        assert_eq!(p.add(Direction::North, 10), Position { x: 100, y: 90 });
        assert_eq!(p.add(Direction::South, 10), (Position { x: 100, y: 110 }));
        assert_eq!(p.add(Direction::West, 10), (Position { x: 90, y: 100 }));
        assert_eq!(p.add(Direction::East, 10), (Position { x: 110, y: 100 }));

        assert_eq!(p.add(Direction::North, -10), (Position { x: 100, y: 110 }));
        assert_eq!(p.add(Direction::South, -10), (Position { x: 100, y: 90 }));
        assert_eq!(p.add(Direction::West, -10), (Position { x: 110, y: 100 }));
        assert_eq!(p.add(Direction::East, -10), (Position { x: 90, y: 100 }));
    }

    #[test]
    fn test_position_distance_to_border() {
        let pos = Position { x: 117, y: 501 };
        assert_eq!(pos.distance_to_border(Direction::North), 1);
        assert_eq!(pos.distance_to_border(Direction::South), 99);
        assert_eq!(pos.distance_to_border(Direction::West), 17);
        assert_eq!(pos.distance_to_border(Direction::East), 83);

        let pos = Position { x: 552, y: 961 };
        assert_eq!(pos.distance_to_border(Direction::North), 61);
        assert_eq!(pos.distance_to_border(Direction::South), 39);
        assert_eq!(pos.distance_to_border(Direction::West), 52);
        assert_eq!(pos.distance_to_border(Direction::East), 48);
    }

    #[test]
    fn test_cell_to_pos() {
        assert_eq!(
            Position::from_cell_position(CellPosition::new(5, 9)),
            Position::new(550, 950)
        );
    }

    #[test]
    fn test_pos_to_cell() {
        assert_eq!(
            Position::new(500, 999).as_cell_pos(),
            CellPosition::new(5, 9)
        );
    }
}
