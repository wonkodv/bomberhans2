#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
struct Time(usize);

#[derive(Debug, Copy, Clone, PartialEq)]
struct PlayerId(usize);

#[derive(Debug, Copy, Clone, PartialEq)]
enum Direction {
    North,
    West,
    South,
    East,
}

#[derive(Debug, Copy, Clone, PartialEq)]
struct CellCoords {
    x: usize,
    y: usize,
}

/// Player positions
#[derive(Debug, Copy, Clone, PartialEq)]
struct PlayerCoords {
    x: isize,
    y: isize,
}

impl PlayerCoords {
    /// Player Coordinates in the middle of a Cell
    fn from_cell_center(cell: CellCoords) -> CellCoords {
        CellCoords {
            x: (cell.x / 1000 + 500).try_into().unwrap(),
            y: (cell.y / 1000 + 500).try_into().unwrap(),
        }
    }

    /// Get the cell Coordinates a player is in.
    fn containing_cell(&self) -> CellCoords {
        CellCoords {
            x: (self.x / 1000).try_into().unwrap(),
            y: (self.y / 1000).try_into().unwrap(),
        }
    }

    /// move position `distance` into  `direction
    ///
    /// ```
    /// assert_eq!(PlayerCoords{100,100}.add(Direction::North,10), PlayerCoords{100,90});
    /// ```
    fn add(&self, direction: Direction, distance: isize) -> PlayerCoords {
        let (x, y) = match direction {
            Direction::North => (0, -distance),
            Direction::West => (-distance, 0),
            Direction::South => (0, distance),
            Direction::East => (distance, 0),
        };
        PlayerCoords {
            x: self.x + x,
            y: self.y + y,
        }
    }
}

/// Ratios of Wood turning into those cell types:
#[derive(Debug)]
struct Ratios {
    power: u8,
    speed: u8,
    bombs: u8,
    schinken: u8,
    teleport: u8,
    wood: u8,
    clear: u8,
}

impl Ratios {
    fn generate(&self, random: u32) -> Cell {
        let sum = self.power
            + self.speed
            + self.bombs
            + self.schinken
            + self.teleport
            + self.wood
            + self.clear;

        let mut random: u8 = (random % (sum as u32)).try_into().unwrap();

        if random < self.power {
            return Cell::PowerUp(PowerUp::Power);
        }
        random -= self.power;

        if random < self.speed {
            return Cell::PowerUp(PowerUp::Speed);
        }
        random -= self.speed;

        if random < self.bombs {
            return Cell::PowerUp(PowerUp::Bombs);
        }
        random -= self.bombs;

        if random < self.schinken {
            return Cell::PowerUp(PowerUp::Schinken);
        }
        random -= self.schinken;

        if random < self.teleport {
            return Cell::Teleport;
        }
        random -= self.teleport;

        if random < self.wood {
            return Cell::Wood;
        }
        random -= self.wood;

        assert!(random < self.clear);
        return Cell::Clear;
    }
}

#[derive(Debug)]
struct Rules {
    ratios: Ratios,
    // how far behind the player the bomb is placed (in 1/1000 of a cell)
    bomb_offset: isize,
    bomb_time: usize,
}

fn random(time: Time, r1: u32, r2: u32) -> u32 {
    let mut x: u32 = 42;
    for i in [time.0 as u32, r1, r2] {
        for b in i.to_le_bytes() {
            x = x.overflowing_mul(31).0.overflowing_add(b as u32).0;
        }
    }
    x
}

#[derive(Debug, Copy, Clone, PartialEq)]
enum PowerUp {
    Speed,
    Power,
    Bombs,
    Schinken,
}

#[derive(Debug, Copy, Clone, PartialEq)]
enum Action {
    Standing,
    Placing,
    Walking,
}

#[derive(Debug, Clone, PartialEq)]
struct PlayerState {
    since: Time,
    action: Action,
    direction: Direction,
}

#[derive(Debug, Clone)]
struct Player {
    name: String,
    id: PlayerId,

    position: PlayerCoords,

    // number of deaths
    deaths: u32,

    // current powerups
    power: u32,
    speed: u32,
    bombs: u32,
    schinken: u32,

    current_bombs_placed: u32,
    // statistics
    total_bombs_placed: u32,
    distance_walked: u32,

    // current State
    state: PlayerState,
}

impl Player {
    fn new(name: String, id: PlayerId) -> Self {
        Self {
            name,
            id,
            position: PlayerCoords { x: 0, y: 0 },
            deaths: 0,
            power: 1,
            speed: 1,
            bombs: 1,
            schinken: 1,
            current_bombs_placed: 0,
            total_bombs_placed: 0,
            distance_walked: 0,
            state: PlayerState {
                since: Time(0),
                action: Action::Standing,
                direction: Direction::South,
            },
        }
    }

    fn eat(&mut self, power_up: PowerUp) {
        match power_up {
            PowerUp::Speed => {
                self.speed += 1;
            }
            PowerUp::Power => {
                self.power += 1;
            }
            PowerUp::Bombs => {
                self.power += 1;
            }
            PowerUp::Schinken => {
                self.power += 1;
            }
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
enum Cell {
    #[default]
    Clear,
    Bomb {
        owner: PlayerId,
        explosion_time: Time,
    },
    Fire,
    HansBurning,
    HansDead,
    HansGore,
    PowerUp(PowerUp),
    Teleport,
    StartPoint,
    Wall,
    Wood,
    WoodBurning(Time),
}

#[derive(Debug, Clone)]
struct Update {
    time: Time,
    player: PlayerId,
    action: Action,
    direction: Direction,
}

#[derive(Debug)]
enum GameError {
    InvalidPlayerId(Update),
}

#[derive(Debug)]
struct Field {
    width: usize,
    height: usize,
    cells: Vec<Cell>,
}

impl Field {
    fn new(width: usize, height: usize) -> Self {
        let cells: Vec<Cell> = (0..width)
            .into_iter()
            .flat_map(|x| {
                (0..height).into_iter().map(move |y| {
                    let x = if x >= width / 2 { width - x - 1 } else { x };
                    let y = if y >= height / 2 { height - y - 1 } else { y };

                    if x + y == 0 {
                        Cell::StartPoint
                    } else if x + y == 1 {
                        Cell::Clear
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

    fn get_mut(&mut self, coords: CellCoords) -> Option<&mut Cell> {
        let c = self.linear_coords(coords);
        self.cells.get_mut(c)
    }

    fn linear_coords(&self, coords: CellCoords) -> usize {
        if coords.x > self.width || coords.y > self.height {
            panic!("coords out of bounds");
        }
        return coords.y * self.width + coords.x;
    }

    fn string_grid(&self) -> String {
        let mut s = String::new();
        for y in 0..self.height {
            for x in 0..self.width {
                let coords = self.linear_coords(CellCoords { x, y });
                let chr = match self.cells[coords] {
                    Cell::Clear => '_',
                    Cell::Bomb { .. } => 'B',
                    Cell::Fire => 'F',
                    Cell::HansBurning => 'X',
                    Cell::HansDead => 'D',
                    Cell::HansGore => 'd',
                    Cell::PowerUp(pu) => match pu {
                        PowerUp::Speed => 'S',
                        PowerUp::Power => 'P',
                        PowerUp::Bombs => 'b',
                        PowerUp::Schinken => 'h',
                    },
                    Cell::Teleport => 'T',
                    Cell::StartPoint => 'O',
                    Cell::Wall => '#',
                    Cell::Wood => '+',
                    Cell::WoodBurning(_) => '+',
                };
                s.push(chr);
            }
            s.push('\n');
        }
        s
    }
}

#[derive(Debug)]
struct Game {
    name: String,
    players: Vec<Player>,
    time: Time,
    field: Field,
    rules: Rules,
}

impl Game {
    fn update(&mut self, update: Update) -> Result<bool, GameError> {
        let player = self
            .players
            .get_mut(update.player.0)
            .ok_or_else(|| GameError::InvalidPlayerId(update.clone()))?;

        player.state = PlayerState {
            since: update.time,
            action: update.action,
            direction: update.direction,
        };
        match update.action {
            Action::Standing => Ok(true),
            Action::Placing => {
                if player.current_bombs_placed >= player.bombs {
                    // GAME RULE: can not place more bombs than you have bomb powerups
                    return Ok(false);
                }

                let cell = self.field.get_mut(
                    player
                        .position
                        .add(player.state.direction, -self.rules.bomb_offset)
                        .containing_cell(),
                );
                let cell = match cell {
                    Some(cell) => cell,
                    None => {
                        // GAME_RULE: placing a Bomb outside of the field is NoOp
                        return Ok(false);
                    }
                };

                // GAME_RULE: placing a bomb onto a powerup gives you that powerup AFTER checking
                // if you have enough bombs to place
                if let Cell::PowerUp(pu) = cell {
                    player.eat(*pu);
                    *cell = Cell::Clear;
                }
                // TODO: placing Bombs into TP and have the Bomb Port would be funny
                // TODO: place Bomb into fire for immediate explosion?
                // GAME_RULE: Bombs can only be placed on empty Cells (after eating any powerups
                // there were)
                if let Cell::Clear = cell {
                    player.total_bombs_placed += 1;
                    player.current_bombs_placed += 1;
                    // GAME_RULE: bomb is owned by the one who placed it
                    // GAME_RULE: bom has fixed timeout (TODO: add randomness?)
                    *cell = Cell::Bomb {
                        owner: player.id,
                        explosion_time: Time(update.time.0 + self.rules.bomb_time),
                    };

                    Ok(true)
                } else {
                    // GAME_RULE: placing a Bomb on Cells not PowerUp or Clear is NoOp.
                    Ok(false)
                }
            }
            Action::Walking => Ok(true),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_player_coords_to_cell() {
        let p = PlayerCoords { x: 5999, y: 9000 };
        assert_eq!(p.containing_cell(), CellCoords { x: 5, y: 9 });
    }

    #[test]
    fn test_player_coord_add() {
        let p = PlayerCoords { x: 100, y: 100 };

        assert_eq!(p.add(Direction::North, 10), PlayerCoords { x: 100, y: 90 });
        assert_eq!(p.add(Direction::South, 10), PlayerCoords { x: 100, y: 110 });
        assert_eq!(p.add(Direction::West, 10), PlayerCoords { x: 90, y: 100 });
        assert_eq!(p.add(Direction::East, 10), PlayerCoords { x: 110, y: 100 });
        assert_eq!(
            p.add(Direction::North, -10),
            PlayerCoords { x: 100, y: 110 }
        );
    }

    #[test]
    fn test_random() {
        let r = random(Time(0), 0, 0);
        assert_eq!(r, random(Time(0), 0, 0));
        assert!(r != random(Time(1), 0, 0));
        assert!(r != random(Time(0), 1, 0));
        assert!(r != random(Time(0), 0, 1));
        assert!(r != random(Time(2), 0, 0));
    }

    //   fn game() -> Game {
    //       let field = Field::new(5, 7);
    //       let rules = Rules {
    //           ratios: todo!(),
    //           bomb_offset: todo!(),
    //           bomb_time: todo!(),
    //       };
    //       let player1 = Player::new("P1".to_string(), PlayerId(4267));
    //       Game {
    //           name: "Match 1".to_owned(),
    //           players: vec![player1],
    //           time: Time(42),
    //           field,
    //           rules,
    //       }
    //   }

    #[test]
    fn test_ratios() {
        let r = Ratios {
            power: 2,
            speed: 2,
            bombs: 2,
            schinken: 2,
            teleport: 2,
            wood: 2,
            clear: 2,
        };

        assert_eq!(Cell::PowerUp(PowerUp::Power), r.generate(000));
        assert_eq!(Cell::PowerUp(PowerUp::Power), r.generate(001));
        assert_eq!(Cell::PowerUp(PowerUp::Speed), r.generate(002));
        assert_eq!(Cell::PowerUp(PowerUp::Speed), r.generate(003));
        assert_eq!(Cell::PowerUp(PowerUp::Bombs), r.generate(004));
        assert_eq!(Cell::PowerUp(PowerUp::Bombs), r.generate(005));
        assert_eq!(Cell::PowerUp(PowerUp::Schinken), r.generate(006));
        assert_eq!(Cell::PowerUp(PowerUp::Schinken), r.generate(007));
        assert_eq!(Cell::Teleport, r.generate(008));
        assert_eq!(Cell::Teleport, r.generate(009));
        assert_eq!(Cell::Wood, r.generate(010));
        assert_eq!(Cell::Wood, r.generate(011));
        assert_eq!(Cell::Clear, r.generate(012));
        assert_eq!(Cell::Clear, r.generate(013));
    }

    #[test]
    fn test_field_gen() {
        let field = Field::new(11, 11);

        assert_eq!(
            field.string_grid(),
            "O_+++++++_O
            _#+#+#+#+#_
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
            .replace(" ", "")
        );
    }
}
