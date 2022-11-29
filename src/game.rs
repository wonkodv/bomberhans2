#[derive(Debug, PartialEq, PartialOrd)]
struct Time(usize);

#[derive(Debug, PartialEq)]
struct PlayerId(usize);

#[derive(Debug, PartialEq)]
enum Direction {
    North,
    West,
    South,
    East,
}

#[derive(Debug, PartialEq)]
struct CellCoords {
    x: usize,
    y: usize,
}

/// Player positions  are calculated in thousands of a Cell
#[derive(Debug)]
struct PlayerCoords {
    x: isize,
    y: isize,
}

impl PlayerCoords {
    fn to_cell(&self) -> CellCoords {
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
struct Ratios {
    power: u8,
    speed: u8,
    bombs: u8,
    teleport: u8,
    schinken: u8,
    wood: u8,
    clear: u8,
}

impl Ratios {
    fn generate(&self, random: u32) -> Cell {
        let sum = self.power
            + self.speed
            + self.bombs
            + self.teleport
            + self.schinken
            + self.wood
            + self.clear;
        let random: u8 = (random % (sum as u32)).try_into().unwrap();

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

        if random < self.teleport {
            return Cell::Teleport;
        }
        random -= self.teleport;

        if random < self.schinken {
            return Cell::PowerUp(PowerUp::Schinken);
        }
        random -= self.schinken;

        if random < self.wood {
            return Cell::Wood;
        }
        random -= self.wood;

        assert!(random < self.clear);
        return Cell::PowerUp(PowerUp::Power);
    }
}

struct Rules {
    ratios: Ratios,
    // how far behind the player the bomb is placed (in 1/1000 of a cell)
    bomb_offset: isize,
    bomb_time: u32,
}

fn random(time: Time, r1: u32, r2: u32) -> u32 {
    let x: u32 = 42;
    for i in [time.0 as u32, r1, r2] {
        for b in i.to_le_bytes() {
            x = x * 31 + b as u32;
        }
    }
    x
}

#[derive(Debug, PartialEq)]
enum PowerUp {
    Speed,
    Power,
    Bombs,
    Schinken,
}

#[derive(Debug, PartialEq)]
enum Action {
    Standing,
    Placing,
    Walking,
}

#[derive(Debug, PartialEq)]
struct PlayerState {
    since: Time,
    action: Action,
    direction: Direction,
}

#[derive(Debug)]
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

#[derive(Debug, Default, PartialEq)]
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
    Startpoint,
    Wall,
    Wood,
    WoodBurning(Time),
}

#[derive(Debug)]
struct Update {
    time: Time,
    player: PlayerId,
    action: Action,
    direction: Direction,
}

#[derive(Debug)]
enum GameError {
    InvalidPlayerId(Update),
    ImpossiblePosition(Player, Update),
}

#[derive(Debug)]
struct Field {
    width: usize,
    height: usize,
    cells: Vec<Cell>,
}

impl Field {
    fn get_mut(&self, index: CellCoords) -> Option<&mut Cell> {
        if index.x > self.width || index.y > self.height {
            panic!("index out of bounds");
        }
        self.cells.get_mut(index.y * self.height + index.x)
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
    fn update(&self, update: Update) -> Result<bool, GameError> {
        let player = self
            .players
            .get_mut(update.player.0)
            .ok_or_else(|| GameError::InvalidPlayerId(update))?;

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
                        .to_cell()
                        .ok_or_else(|| GameError::ImpossiblePosition(player, update))?,
                );
                if let Cell::PowerUp(pu) = cell {
                    //
                    player.eat(pu);
                    *cell = Cell::Clear;
                }
                // TODO: placing Bombs into TP would be funny
                if cell == Cell::Clear {
                    player.current_bombs_placed += 1;
                    // GAME_RULE: bomb is owned by the one who placed it
                    // GAME_RULE: bom has fixed timeout (TODO: add randomness?)
                    *cell = Cell::Bomb {
                        owner: player.id,
                        explosion_time: update.time + self.rules.bomb_time,
                    };
                }
            }
            Action::Walking => Ok(true),
        }
    }
}
