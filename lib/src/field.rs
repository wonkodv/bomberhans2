use core::fmt;
use std::ops::Index;
use std::ops::IndexMut;

use crate::settings::Settings;
use crate::utils::CellPosition;
use crate::utils::GameTime;
use crate::utils::PlayerId;

#[derive(Copy, Clone, PartialEq)]
pub enum Upgrade {
    Speed,
    Power,
    Bombs,
}

impl fmt::Debug for Upgrade {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Upgrade::Speed => write!(f, "👟"),
            Upgrade::Power => write!(f, "💪"),
            Upgrade::Bombs => write!(f, "💣"),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub enum Cell {
    #[default]
    Empty,
    Bomb {
        owner: PlayerId,
        power: u32,
        expire: GameTime,
    },
    Fire {
        owner: PlayerId,
        expire: GameTime,
    },
    TombStone(PlayerId),
    Upgrade(Upgrade),
    Teleport,
    StartPoint,
    Wall,
    Wood,
    WoodBurning {
        expire: GameTime,
    },
}

impl Cell {
    pub fn to_char(&self) -> char {
        // TODO: how about these: _ 💣 💥 🪦 🏃 💪 🧨 🚪 🏳 🧱 🪜 🔥
        match *self {
            Cell::Empty => '_',
            Cell::Bomb { .. } => 'B',
            Cell::Fire { .. } => 'F',
            Cell::TombStone(..) => 'D',
            Cell::Upgrade(pu) => match pu {
                Upgrade::Speed => 's',
                Upgrade::Power => 'p',
                Upgrade::Bombs => 'b',
            },
            Cell::Teleport => 'T',
            Cell::StartPoint => 'O',
            Cell::Wall => '#',
            Cell::Wood => '+',
            Cell::WoodBurning { .. } => 'W',
        }
    }

    pub fn from_char(chr: char) -> Result<Self, String> {
        let owner = PlayerId(0);
        let power = 3;
        let expire = GameTime::default(); // everything expires on 1st tick

        let cell = match chr {
            '_' => Cell::Empty,
            'B' => Cell::Bomb {
                owner,
                power,
                expire,
            },
            'F' => Cell::Fire { owner, expire },
            'D' => Cell::TombStone(owner),
            's' => Cell::Upgrade(Upgrade::Speed),
            'p' => Cell::Upgrade(Upgrade::Power),
            'b' => Cell::Upgrade(Upgrade::Bombs),
            'T' => Cell::Teleport,
            'O' => Cell::StartPoint,
            '#' => Cell::Wall,
            '+' => Cell::Wood,
            'W' => Cell::WoodBurning { expire },
            chr => return Err(format!("Invalid character {chr}")),
        };
        Ok(cell)
    }

    pub fn name(&self) -> &'static str {
        match *self {
            Cell::Empty => "empty",
            Cell::Bomb { .. } => "bomb",
            Cell::Fire { .. } => "fire",
            Cell::TombStone(..) => "tomb_stone",
            Cell::Upgrade(upgrade) => match upgrade {
                Upgrade::Speed => "upgrade_speed",
                Upgrade::Power => "upgrade_power",
                Upgrade::Bombs => "upgrade_bomb",
            },
            Cell::Teleport => "teleport",
            Cell::StartPoint => "start_point",
            Cell::Wall => "wall",
            Cell::Wood => "wood",
            Cell::WoodBurning { .. } => "wood_burning",
        }
    }

    pub fn walkable(&self) -> bool {
        match *self {
            Cell::Empty
            | Cell::Bomb { .. }
            | Cell::Fire { .. }
            | Cell::TombStone(..)
            | Cell::Upgrade(_)
            | Cell::Teleport
            | Cell::StartPoint => true,
            Cell::Wall | Cell::Wood | Cell::WoodBurning { .. } => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    pub width: u32,
    pub height: u32,
    pub cells: Vec<Cell>,
}

impl Field {
    pub fn new(width: u32, height: u32) -> Self {
        let cells: Vec<Cell> = (0..height)
            .flat_map(|y| {
                (0..width).map(move |x| {
                    let x = if x >= width / 2 { width - x - 1 } else { x };
                    let y = if y >= height / 2 { height - y - 1 } else { y };

                    if x == 0 && y == 0 {
                        Cell::StartPoint
                    } else if x + y == 1 {
                        Cell::Empty
                    } else if (x % 2) == 1 && (y % 2) == 1 {
                        Cell::Wall
                    } else {
                        Cell::Wood
                    }
                })
            })
            .collect();

        Self {
            width,
            height,
            cells,
        }
    }

    pub fn new_from_rules(settings: &Settings) -> Self {
        Self::new(settings.width, settings.height)
    }

    pub fn is_cell_in_field(&self, cell: CellPosition) -> bool {
        cell.x >= 0 && cell.y >= 0 && cell.x < self.width as i32 && cell.y < self.height as i32
    }

    pub fn string_grid(&self) -> String {
        let mut s = String::new();
        for y in 0..self.height as i32 {
            for x in 0..self.width as i32 {
                let cell = &self[CellPosition::new(x, y)];
                s.push(cell.to_char());
            }
            s.push('\n');
        }
        s
    }

    pub fn new_from_string_grid(string: &str) -> Result<Self, String> {
        let lines: Vec<&str> = string
            .split('\n')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect();
        if lines.is_empty() {
            return Err("0 rows".to_owned());
        }
        let height = lines.len();
        let width = lines[0].len();

        for (i, row) in lines.iter().enumerate() {
            if row.len() != width {
                return Err(format!("line {i} has wrong length"));
            }
        }

        let cells: Vec<Cell> = lines
            .iter()
            .enumerate()
            .flat_map(|(y, row)| {
                row.chars().enumerate().map(move |(x, chr)| {
                    Cell::from_char(chr)
                        .map_err(|e| format!("Character for Cell {x}/{y} invalid: {e}"))
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self {
            width: width
                .try_into()
                .map_err(|err: std::num::TryFromIntError| err.to_string())?,
            height: height
                .try_into()
                .map_err(|err: std::num::TryFromIntError| err.to_string())?,
            cells,
        })
    }

    pub fn iter(&self) -> impl Iterator<Item = (CellPosition, &Cell)> {
        self.iter_indices().map(move |pos| (pos, &self[pos]))
    }

    pub fn iter_indices(&self) -> impl Iterator<Item = CellPosition> {
        let height = self.height;
        (0..self.width as i32)
            .flat_map(move |x| (0..height as i32).map(move |y| CellPosition::new(x, y)))
    }

    pub fn iter_with_border(&self) -> impl Iterator<Item = (CellPosition, &Cell)> {
        self.iter_indices_with_border()
            .map(move |pos| (pos, &self[pos]))
    }
    pub fn iter_indices_with_border(&self) -> impl Iterator<Item = CellPosition> {
        let height = self.height;
        (-1..(self.width + 1) as i32)
            .flat_map(move |x| (-1..(height + 1) as i32).map(move |y| CellPosition::new(x, y)))
    }

    pub fn start_positions(&self) -> Vec<CellPosition> {
        self.iter()
            .filter_map(|(pos, cell)| {
                if *cell == Cell::StartPoint {
                    Some(pos)
                } else {
                    None
                }
            })
            .collect()
    }
}

impl Index<CellPosition> for Field {
    type Output = Cell;

    fn index(&self, index: CellPosition) -> &Self::Output {
        if self.is_cell_in_field(index) {
            &self.cells
                [usize::try_from(index.y * self.width as i32 + index.x).expect("index fits usize")]
        } else {
            &Cell::Wall
        }
    }
}

impl IndexMut<CellPosition> for Field {
    fn index_mut(&mut self, index: CellPosition) -> &mut Self::Output {
        if self.is_cell_in_field(index) {
            &mut self.cells
                [usize::try_from(index.y * self.width as i32 + index.x).expect("index fits usize")]
        } else {
            panic!("y > height: {} > {}", index.y, self.height)
        }
    }
}

struct FieldMutIterator<'f> {
    field: &'f mut Field,
    pos: CellPosition,
}
impl<'f> FieldMutIterator<'f> {
    fn new(field: &'f mut Field) -> Self {
        Self {
            field,
            pos: CellPosition::new(0, 0),
        }
    }
}
#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_pos_in_field() {
        let field = Field::new(11, 11);
        assert!(field.is_cell_in_field(CellPosition::new(10, 10)));
        assert!(!field.is_cell_in_field(CellPosition::new(10, 11)));
        assert!(!field.is_cell_in_field(CellPosition::new(11, 10)));
    }

    #[test]
    fn test_field_gen() {
        let field = Field::new(11, 13);

        println!("{}", field.string_grid());
        assert_eq!(
            field.string_grid(),
            "
                O_+++++++_O
                _#+#+#+#+#_
                +++++++++++
                +#+#+#+#+#+
                +++++++++++
                +#+#+#+#+#+
                +++++++++++
                +#+#+#+#+#+
                +++++++++++
                +#+#+#+#+#+
                +++++++++++
                _#+#+#+#+#_
                O_+++++++_O
            "
            .trim_start()
            .replace(' ', "")
        );
    }

    #[test]
    fn test_field_from_string() {
        let expected = "
            O_+++++++_O
            _#+#+#+#+#_
            spb++++++++
            +#+#+#+#+#+
            +++++++++++
            +#+#+#+#+#+
            +++++++++++
            +#+#+#+#+#+
            +++++++++++
            _#+#+#+#+#_
            O_+++++++_O
            "
        .trim_start()
        .replace(' ', "");
        let actual = Field::new_from_string_grid(&expected)
            .unwrap()
            .string_grid();
        dbg!(&actual);
        dbg!(&expected);
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_generated_with_start_points() {
        let field = Field::new(17, 13);
        println!("{}", field.string_grid());
        assert_eq!(
            field.start_positions(),
            vec![
                CellPosition { x: 0, y: 0 },
                CellPosition { x: 0, y: 12 },
                CellPosition { x: 16, y: 0 },
                CellPosition { x: 16, y: 12 }
            ]
        );
    }
}
