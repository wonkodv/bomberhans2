use std::collections::HashSet;
use std::error::Error;
use std::fmt;
use std::ops::Index;
use std::ops::IndexMut;
use std::rc::Rc;

pub const TICKS_PER_SECOND: u32 = 60;

type HResult<T> = Result<T, String>;

#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
pub struct Time(u32);

impl Time {
    pub fn from_seconds(seconds: u32) -> Self {
        Self(seconds * TICKS_PER_SECOND)
    }

    pub fn seconds(self) -> u32 {
        self.0 / TICKS_PER_SECOND
    }
    pub fn ticks(self) -> u32 {
        self.0
    }
}

impl std::ops::Add<Time> for Time {
    type Output = Time;

    fn add(self, rhs: Time) -> Self::Output {
        Time(self.0 + rhs.0)
    }
}

impl std::ops::Add<i32> for Time {
    type Output = Time;

    fn add(self, rhs: i32) -> Self::Output {
        Time(
            self.0
                .checked_add_signed(rhs)
                .expect("time doesn't overflow"),
        )
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct PlayerId(usize);

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Direction {
    North,
    West,
    South,
    East,
}

/// Index of a cell
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct CellPosition {
    pub x: u32,
    pub y: u32,
}

impl CellPosition {
    fn new(x: u32, y: u32) -> Self {
        Self { x, y }
    }

    /// Distance if in line of fire
    fn fire_distance(&self, other: &Self) -> Option<u32> {
        if self.x == other.x {
            Some(self.y.abs_diff(other.y))
        } else if self.y == other.y {
            Some(self.y.abs_diff(other.y))
        } else {
            None
        }
    }
}

/// Player positions
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Position {
    pub x: u32,
    pub y: u32,
}

impl Position {
    /// Player position is tracked in this many fractions of a cell
    pub const PLAYER_POSITION_ACCURACY: u32 = 100;

    fn new(x: u32, y: u32) -> Self {
        Self { x, y }
    }

    /// move position `distance` into  `direction
    fn add(&self, direction: Direction, distance: i32) -> Option<Self> {
        let Self { x, y } = *self;
        let (x, y) = match direction {
            Direction::North => (x, y.checked_add_signed(-distance)?),
            Direction::West => (x.checked_add_signed(-distance)?, y),
            Direction::South => (x, y.checked_add_signed(distance)?),
            Direction::East => (x.checked_add_signed(distance)?, y),
        };
        Some(Self::new(x, y))
    }

    fn as_cell_pos(&self) -> CellPosition {
        CellPosition {
            x: self.x / Self::PLAYER_POSITION_ACCURACY,
            y: self.y / Self::PLAYER_POSITION_ACCURACY,
        }
    }

    fn from_cell_position(p: CellPosition) -> Self {
        Self {
            x: p.x * Self::PLAYER_POSITION_ACCURACY + Self::PLAYER_POSITION_ACCURACY / 2,
            y: p.y * Self::PLAYER_POSITION_ACCURACY + Self::PLAYER_POSITION_ACCURACY / 2,
        }
    }
}

/// Ratios of Wood turning into those cell types:
#[derive(Debug, Clone)]
pub struct Ratios {
    power: u8,
    speed: u8,
    bombs: u8,
    teleport: u8,
    wood: u8,
    clear: u8,
}

impl Default for Ratios {
    fn default() -> Self {
        Self::new(8, 9, 7, 1, 1, 20)
    }
}

impl Ratios {
    pub fn new(power: u8, speed: u8, bombs: u8, teleport: u8, wood: u8, clear: u8) -> Self {
        Self {
            power,
            speed,
            bombs,
            teleport,
            wood,
            clear,
        }
    }

    fn generate(&self, random: usize) -> Cell {
        let sum: u8 = self.power + self.speed + self.bombs + self.teleport + self.wood + self.clear;

        let mut random: u8 = (random % (usize::from(sum)))
            .try_into()
            .expect("random % sum fits");

        if random < self.power {
            return Cell::Upgrade(Upgrade::Power);
        }
        random -= self.power;

        if random < self.speed {
            return Cell::Upgrade(Upgrade::Speed);
        }
        random -= self.speed;

        if random < self.bombs {
            return Cell::Upgrade(Upgrade::Bombs);
        }
        random -= self.bombs;

        if random < self.teleport {
            return Cell::Teleport;
        }
        random -= self.teleport;

        if random < self.wood {
            return Cell::Wood;
        }
        random -= self.wood;

        assert!(random < self.clear);
        return Cell::Empty;
    }
}

#[derive(Debug, Clone)]
pub struct Rules {
    // field width
    pub width: u32,

    // field width
    pub height: u32,

    pub players: u32,

    /// Ratios what comes out of burned down walls
    pub ratios: Ratios,

    /// how far behind the player the bomb is placed [cell/100]
    pub bomb_offset: i32,

    /// time after bomb placement that the bomb explodes
    pub bomb_time: Time,

    /// player walking speed at initial upgrade [cells/100/s]
    pub speed_multiplyer: u32,

    /// player walking speed upgrade start value
    pub speed_offset: u32,

    /// percentage that walking on bomb succeeds each update
    pub bomb_walking_chance: u8,

    /// Power of Upgrade Paackets exploding
    pub upgrade_explosion_power: u8,

    /// how long before burning wood turns into something
    pub wood_burn_time: Time,

    /// how long fire burns
    pub fire_burn_time: Time,
}

impl Default for Rules {
    fn default() -> Self {
        Rules {
            width: 17,
            height: 13,
            players: 4,
            ratios: Ratios::default(),
            bomb_offset: 30,
            bomb_time: Time::from_seconds(3),
            speed_multiplyer: 10,
            speed_offset: 42,
            bomb_walking_chance: 70,
            upgrade_explosion_power: 1,
            wood_burn_time: Time::from_seconds(3),
            fire_burn_time: Time::from_seconds(3),
        }
    }
}

impl Rules {
    fn get_update_walk_distance(&self, player_speed: u8) -> u32 {
        (self.speed_offset + u32::from(player_speed)) * self.speed_multiplyer
    }
}

fn random(time: Time, r1: u32, r2: u32) -> usize {
    let mut x: usize = 42;
    for i in [time.0, r1, r2] {
        for b in i.to_le_bytes() {
            x = x.overflowing_mul(31).0.overflowing_add(b.into()).0;
        }
    }
    x
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Upgrade {
    Speed,
    Power,
    Bombs,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Action {
    Standing,
    Placing,
    Walking,
}

#[derive(Debug, Clone)]
pub struct Player {
    /// Name the player chose
    name: String,

    /// Id of the player in the game
    id: PlayerId,

    /// Re-/Spawn place
    start_position: Position,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlayerState {
    /// current position
    pub position: Position,

    /// number of deaths since the game started
    pub deaths: u32,

    /// number of kills since the game started
    pub kills: u32,

    /// current bomb power upgrades
    pub power: u8,

    /// current walking speed upgrades
    pub speed: u8,

    /// current bomb capacity upgrades
    pub bombs: u8,

    /// current placed bombs. Increased when placing, decreased when exploding.
    pub current_bombs_placed: u8,

    /// current action
    pub action: Action,

    /// current direction
    pub direction: Direction,
    // TODO: track total walking distance, total bombs, ...
}

impl PlayerState {
    fn new(position: Position) -> Self {
        Self {
            position,
            deaths: 0,
            kills: 0,
            power: 1,
            speed: 1,
            bombs: 0,
            current_bombs_placed: 0,
            action: Action::Standing,
            direction: Direction::South,
        }
    }

    fn eat(&mut self, upgrade: Upgrade) {
        let up = match upgrade {
            Upgrade::Speed => &mut self.speed,
            Upgrade::Power => &mut self.power,
            Upgrade::Bombs => &mut self.bombs,
        };
        *up = up.saturating_add(1);
    }

    fn die(&mut self, _killed_by: PlayerId, start_position: Position) {
        self.power = 1;
        self.speed = 1;
        self.bombs = 1;
        self.position = start_position;
        self.current_bombs_placed = 0;
        self.action = Action::Standing;
        self.direction = Direction::South;
    }

    fn score(&mut self, _killed: PlayerId) {
        self.kills += 1;
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub enum Cell {
    #[default]
    Empty,
    Bomb {
        owner: PlayerId,
        power: u8,
        expire: Time,
    },
    Fire {
        owner: PlayerId,
        expire: Time,
    },
    TombStone,
    Upgrade(Upgrade),
    Teleport,
    StartPoint,
    Wall,
    Wood,
    WoodBurning(Time),
}

impl Cell {
    pub fn to_char(&self) -> char {
        // TODO: how about these: _ 💣 💥 🪦 🏃 💪 🧨 🚪 🏳 🧱 🪜 🔥
        match self {
            Cell::Empty => '_',
            Cell::Bomb { .. } => 'B',
            Cell::Fire { .. } => 'F',
            Cell::TombStone => 'D',
            Cell::Upgrade(pu) => match pu {
                Upgrade::Speed => 's',
                Upgrade::Power => 'p',
                Upgrade::Bombs => 'b',
            },
            Cell::Teleport => 'T',
            Cell::StartPoint => 'O',
            Cell::Wall => '#',
            Cell::Wood => '+',
            Cell::WoodBurning(_) => 'W',
        }
    }

    pub fn from_char(chr: char) -> HResult<Self> {
        let owner = PlayerId(0);
        let power = 1;
        let expire = Time(1);
        let cell = match chr {
            '_' => Cell::Empty,
            'B' => Cell::Bomb {
                owner,
                power,
                expire,
            },
            'F' => Cell::Fire { owner, expire },
            'D' => Cell::TombStone,
            's' => Cell::Upgrade(Upgrade::Speed),
            'p' => Cell::Upgrade(Upgrade::Power),
            'b' => Cell::Upgrade(Upgrade::Bombs),
            'T' => Cell::Teleport,
            'O' => Cell::StartPoint,
            '#' => Cell::Wall,
            '+' => Cell::Wood,
            'W' => Cell::WoodBurning(expire),
            chr => return Err(format!("Invalid character {}", chr)),
        };
        Ok(cell)
    }

    pub fn name(&self) -> &'static str {
        match self {
            Cell::Empty => "empty",
            Cell::Bomb { .. } => "bomb",
            Cell::Fire { .. } => "fire",
            Cell::TombStone => "tomb_stone",
            Cell::Upgrade(upgrade) => match upgrade {
                Upgrade::Speed => "upgrade_speed",
                Upgrade::Power => "upgrade_speed",
                Upgrade::Bombs => "upgrade_speed",
            },
            Cell::Teleport => "teleport",
            Cell::StartPoint => "start_point",
            Cell::Wall => "wall",
            Cell::Wood => "wood",
            Cell::WoodBurning(_) => "wood_burning",
        }
    }
}

#[derive(Debug)]
pub struct Field {
    width: u32,
    height: u32,
    cells: Vec<Cell>,
}

impl Field {
    fn new(width: u32, height: u32) -> Self {
        assert!(width % 2 == 1);
        assert!(height % 2 == 1);
        let cells: Vec<Cell> = (0..height)
            .into_iter()
            .flat_map(|y| {
                (0..width).into_iter().map(move |x| {
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

    pub fn is_cell_in_field(&self, cell: CellPosition) -> bool {
        if cell.x >= self.width {
            false
        } else if cell.y >= self.height {
            false
        } else {
            true
        }
    }

    pub fn string_grid(&self) -> String {
        let mut s = String::new();
        for y in 0..self.height {
            for x in 0..self.width {
                let cell = &self[CellPosition::new(x, y)];
                s.push(cell.to_char());
            }
            s.push('\n');
        }
        s
    }

    pub fn from_string_grid(string: &str) -> HResult<Self> {
        let lines: Vec<&str> = string
            .split('\n')
            .map(|s| s.trim())
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
                        .map_err(|e| format!("Character for Cell {x}/{y} invalid: {chr}"))
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
        return self.iter_indices().map(move |pos| (pos, &self[pos]));
    }

    pub fn iter_indices(&self) -> impl Iterator<Item = CellPosition> {
        let height = self.height;
        return (0..self.width).into_iter().flat_map(move |x| {
            (0..height)
                .into_iter()
                .map(move |y| CellPosition::new(x, y))
        });
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
            &self.cells[usize::try_from(index.y * self.width + index.x).expect("index fits usize")]
        } else {
            panic!("y > height: {} > {}", index.y, self.height)
        }
    }
}

impl IndexMut<CellPosition> for Field {
    fn index_mut(&mut self, index: CellPosition) -> &mut Self::Output {
        if self.is_cell_in_field(index) {
            &mut self.cells
                [usize::try_from(index.y * self.width + index.x).expect("index fits usize")]
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

pub struct Update {
    time: Time,
    events: Vec<Event>,
}

pub enum Event {
    Move(PlayerId, Position),
    Eat(PlayerId, Upgrade, CellPosition),
    Killed {
        dead: PlayerId,
        owner: PlayerId,
        at: CellPosition,
    },
    Place {
        player_id: PlayerId,
        cell: CellPosition,
        expire: Time,
        power: u8,
    },
    Teleport {
        player_id: PlayerId,
        from: CellPosition,
        to: CellPosition,
    },
    StateChange {
        player_id: PlayerId,
        action: Action,
        direction: Direction,
    },
    CellUpdate(CellPosition, Cell),
}

#[derive(Debug)]
pub struct GameStatic {
    pub name: String,
    pub players: Vec<Player>,
    pub rules: Rules,
}

pub struct Game {
    pub game_static: Rc<GameStatic>,
    pub game_state: GameState,
    pub local_player: PlayerId,
}

impl Game {
    pub fn new_local_game(name: String, rules: Rules) -> Self {
        let field = Field::new(rules.width, rules.height);
        let start_positions = field.start_positions();

        assert!(start_positions.len() <= rules.players as _);

        let local_player = PlayerId(1);

        let players: Vec<Player> = (0..(rules.players as usize))
            .into_iter()
            .map(|id| Player {
                name: {
                    if id == local_player.0 as _ {
                        format!("Player {}", id)
                    } else {
                        "Local Player".into()
                    }
                },
                id: PlayerId(id as _),
                start_position: Position::from_cell_position(start_positions[id]),
            })
            .collect();

        let player_states = players
            .iter()
            .map(|p| PlayerState::new(p.start_position))
            .collect();

        let game_static = GameStatic {
            name,
            players,
            rules,
        };
        let game_static = Rc::new(game_static);

        let game_state = GameState {
            time: Time(0),
            field,
            player_states,
            game_static: Rc::clone(&game_static),
        };
        Self {
            game_static,
            game_state,
            local_player,
        }
    }
}

#[derive(Debug)]
pub struct GameState {
    pub time: Time,
    pub field: Field,
    pub player_states: Vec<PlayerState>,
    pub game_static: Rc<GameStatic>,
}

impl GameState {
    fn new(
        time: Time,
        field: Field,
        player_states: Vec<PlayerState>,
        game: Rc<GameStatic>,
    ) -> Self {
        Self {
            time,
            field,
            player_states,
            game_static: game,
        }
    }

    /// advance a player 1 tick and generate the events from that
    fn player_update_event(&self, player_id: PlayerId) -> Vec<Event> {
        let mut events: Vec<Event> = Vec::new();
        let player = &self.game_static.players[player_id.0];
        let player_state = &self.player_states[player_id.0];

        match player_state.action {
            Action::Standing | Action::Placing => { /*nothing to do */ }
            Action::Walking => {
                let position = player_state.position.add(
                    player_state.direction,
                    self.game_static
                        .rules
                        .get_update_walk_distance(player_state.speed)
                        .try_into()
                        .expect("walked distance fits i32"),
                );
                if let Some(position) = position {
                    let cell_position = position.as_cell_pos();
                    if self.field.is_cell_in_field(cell_position) {
                        let cell = &self.field[cell_position];
                        match cell {
                            Cell::StartPoint | Cell::Empty => {
                                events.push(Event::Move(player_id, position));
                            }
                            Cell::Bomb { .. } | Cell::TombStone => {
                                if random(self.time, position.x, position.y) % 100
                                    < self.game_static.rules.bomb_walking_chance.into()
                                {
                                    // GAME_RULE: walking on bombs randomly happens or doesn't, decided
                                    // each update.
                                    events.push(Event::Move(player_id, position));
                                }
                            }
                            Cell::Fire { owner, .. } => {
                                events.push(Event::Killed {
                                    dead: player_id,
                                    owner: *owner,
                                    at: cell_position,
                                });
                            }
                            Cell::Upgrade(upgrade) => {
                                events.push(Event::Move(player_id, position));
                                events.push(Event::Eat(player_id, *upgrade, cell_position));
                            }
                            Cell::Teleport => {
                                let targets: Vec<(CellPosition, &Cell)> = self
                                    .field
                                    .iter()
                                    .filter(|(pos, cell)| {
                                        **cell == Cell::Teleport && *pos != cell_position
                                    })
                                    .collect();
                                if targets.len() > 1 {
                                    let target = targets
                                        [random(self.time, position.x, position.y) % targets.len()];
                                    let (to, target_cell): (_, &Cell) = target;
                                    assert_eq!(*target_cell, Cell::Teleport);
                                    events.push(Event::Teleport {
                                        player_id,
                                        from: cell_position,
                                        to,
                                    });
                                } else {
                                    events.push(Event::Move(player_id, position));
                                }
                            }
                            Cell::Wall | Cell::Wood | Cell::WoodBurning(_) => {} /* no walking through walls */
                        }
                    }
                }
            }
        };
        events
    }

    fn apply_event(&mut self, events: &[Event]) {
        for event in events {
            match event {
                Event::Move(player, position) => {
                    self.player_states[player.0].position = *position;
                }
                Event::Eat(player, upgrade, position) => {
                    self.player_states[player.0].eat(*upgrade);
                    self.field[*position] = Cell::Empty;
                }
                Event::Killed { dead, owner, at } => {
                    self.player_states[dead.0]
                        .die(*owner, self.game_static.players[dead.0].start_position);
                    self.field[*at] = Cell::TombStone;
                    self.player_states[owner.0].score(*dead);
                }
                Event::Place {
                    player_id,
                    cell,
                    expire,
                    power,
                } => {
                    let player = &mut self.player_states[player_id.0];
                    let cell = &mut self.field[*cell];

                    assert!(player.current_bombs_placed < player.bombs);
                    player.current_bombs_placed += 1;

                    assert!(*cell == Cell::Empty);
                    *cell = Cell::Bomb {
                        owner: *player_id,
                        expire: *expire,
                        power: *power,
                    };
                }
                Event::Teleport {
                    player_id,
                    from,
                    to,
                } => {
                    let player_state = &mut self.player_states[player_id.0];
                    player_state.position = Position::from_cell_position(*to);

                    assert_eq!(self.field[*from], Cell::Teleport);
                    assert_eq!(self.field[*to], Cell::Teleport);

                    self.field[*from] = Cell::Empty;
                    self.field[*to] = Cell::Empty;
                }
                Event::StateChange {
                    player_id,
                    action,
                    direction,
                } => {
                    let player_state = &mut self.player_states[player_id.0];
                    player_state.action = *action;
                    player_state.direction = *direction;
                }
                Event::CellUpdate(pos, cell) => {
                    self.field[*pos] = cell.clone();
                }
            }
        }
    }

    /// set a cell on fire.
    ///
    /// consider_tp if target is a teleport, explode a random other teleport too.
    ///
    /// returns if the should continue further
    fn set_on_fire(&mut self, cell: CellPosition, owner: PlayerId, consider_tp: bool) -> bool {
        let (burns, power) = match &self.field[cell] {
            Cell::Fire { .. } | Cell::Empty | Cell::TombStone => (true, 0),
            Cell::Bomb { power, .. } => (true, *power),
            Cell::Upgrade(_) => (true, self.game_static.rules.upgrade_explosion_power),
            Cell::Teleport => {
                if consider_tp {
                    let ports: Vec<CellPosition> = self
                        .field
                        .iter()
                        .filter_map(|(i_pos, i_cell)| {
                            if *i_cell == Cell::Teleport && i_pos != cell {
                                Some(i_pos)
                            } else {
                                None
                            }
                        })
                        .collect();
                    if !ports.is_empty() {
                        let other = ports[random(self.time, cell.x, cell.y) % ports.len()];
                        self.set_on_fire(other, owner, false);
                    }
                }
                (true, self.game_static.rules.upgrade_explosion_power)
            }
            Cell::StartPoint | Cell::WoodBurning(_) | Cell::Wall => (false, 0),
            Cell::Wood => {
                self.field[cell] = Cell::WoodBurning(self.game_static.rules.wood_burn_time);
                (false, 0)
            }
        };
        if burns {
            let power: u32 = power.into();
            self.field[cell] = Cell::Fire {
                owner,
                expire: self.game_static.rules.fire_burn_time,
            };
            if power > 0 {
                for i in 1..=power {
                    if !self.set_on_fire(CellPosition::new(cell.x + i, cell.y), owner, true) {
                        break;
                    };
                }
                for i in 1..=power {
                    if !self.set_on_fire(CellPosition::new(cell.x - i, cell.y), owner, true) {
                        break;
                    }
                }
                for i in 1..=power {
                    if !self.set_on_fire(CellPosition::new(cell.x, cell.y + i), owner, true) {
                        break;
                    }
                }
                for i in 1..=power {
                    if !self.set_on_fire(CellPosition::new(cell.x, cell.y - i), owner, true) {
                        break;
                    }
                }
            }
        }
        burns
    }

    fn field_update_events(&mut self) {
        let fields_on_fire: HashSet<CellPosition> = HashSet::new();

        for cell_idx in self.field.iter_indices() {
            let cell = &mut self.field[cell_idx];
            match cell {
                Cell::Bomb {
                    owner,
                    power,
                    expire,
                } => {
                    expire.0 -= 1;
                    if expire.0 == 0 {
                        let owner = *owner;
                        self.set_on_fire(CellPosition::new(cell_idx.x, cell_idx.y), owner, true);
                    }
                }
                Cell::Fire { owner, expire } => {
                    expire.0 -= 1;
                    if expire.0 == 0 {
                        *cell = Cell::Empty;
                    }
                }
                Cell::WoodBurning(_) => {
                    let r = random(self.time, cell_idx.x, cell_idx.y);
                    *cell = self.game_static.rules.ratios.generate(r);
                }

                Cell::TombStone
                | Cell::Upgrade(_)
                | Cell::Teleport
                | Cell::StartPoint
                | Cell::Empty
                | Cell::Wall
                | Cell::Wood => {}
            }
        }
    }

    pub fn update(&mut self) {
        self.time.0 += 1;
        for i in 0..self.player_states.len() {
            self.player_update_event(PlayerId(i));
        }
    }

    pub fn player_action(
        &self,
        player_id: PlayerId,
        action: Action,
        direction: Direction,
    ) -> Vec<Event> {
        let mut events = Vec::new();
        let player = &self.player_states[player_id.0];

        if player.action != action || player.direction != direction {
            match action {
                Action::Standing | Action::Walking => events.push(Event::StateChange {
                    player_id,
                    action,
                    direction,
                }),
                Action::Placing => {
                    // GAME RULE: can not place more bombs than you have bomb powerups
                    if player.current_bombs_placed != player.bombs {
                        // log out of bombs
                    } else {
                        let position = player
                            .position
                            .add(player.direction, -self.game_static.rules.bomb_offset);

                        if let Some(position) = position {
                            let cell_position = position.as_cell_pos();
                            if self.field.is_cell_in_field(cell_position) {
                                let cell = &self.field[cell_position];

                                // GAME_RULE: placing a bomb onto a powerup gives you that powerup AFTER checking
                                // if you have enough bombs to place
                                if let Cell::Upgrade(upgrade) = cell {
                                    events.push(Event::Eat(player_id, *upgrade, cell_position));
                                }

                                // TODO: placing Bombs into TP and have the Bomb Port would be funny
                                // TODO: place Bomb into fire for immediate explosion?
                                // GAME_RULE: Bombs can only be placed on empty Cells (after eating any powerups
                                // there were)
                                if Cell::Empty == *cell {
                                    events.push(Event::Place {
                                        player_id,
                                        cell: cell_position,
                                        expire: self.time + self.game_static.rules.bomb_time,
                                        power: player.power,
                                    });
                                }
                            } else {
                                // TODO: log not placing bomb from here
                            }
                        } else {
                            // TODO: log not placing bomb from here
                        }
                    }
                }
            }
        }

        events
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_player_coord_add() {
        let p = Position { x: 100, y: 100 };

        assert_eq!(p.add(Direction::North, 101), None);

        assert_eq!(
            p.add(Direction::North, 10),
            Some(Position { x: 100, y: 90 })
        );
        assert_eq!(
            p.add(Direction::South, 10),
            Some(Position { x: 100, y: 110 })
        );
        assert_eq!(p.add(Direction::West, 10), Some(Position { x: 90, y: 100 }));
        assert_eq!(
            p.add(Direction::East, 10),
            Some(Position { x: 110, y: 100 })
        );
    }

    #[test]
    fn test_player_coord_sub() {
        let p = Position { x: 100, y: 100 };

        assert_eq!(
            p.add(Direction::North, -10),
            Some(Position { x: 100, y: 110 })
        );
        assert_eq!(
            p.add(Direction::South, -10),
            Some(Position { x: 100, y: 90 })
        );
        assert_eq!(
            p.add(Direction::West, -10),
            Some(Position { x: 110, y: 100 })
        );
        assert_eq!(
            p.add(Direction::East, -10),
            Some(Position { x: 90, y: 100 })
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
        let r = Ratios::new(2, 2, 2, 2, 2, 2);

        assert_eq!(Cell::Upgrade(Upgrade::Power), r.generate(000));
        assert_eq!(Cell::Upgrade(Upgrade::Power), r.generate(001));
        assert_eq!(Cell::Upgrade(Upgrade::Speed), r.generate(002));
        assert_eq!(Cell::Upgrade(Upgrade::Speed), r.generate(003));
        assert_eq!(Cell::Upgrade(Upgrade::Bombs), r.generate(004));
        assert_eq!(Cell::Upgrade(Upgrade::Bombs), r.generate(005));
        assert_eq!(Cell::Teleport, r.generate(006));
        assert_eq!(Cell::Teleport, r.generate(007));
        assert_eq!(Cell::Wood, r.generate(008));
        assert_eq!(Cell::Wood, r.generate(009));
        assert_eq!(Cell::Empty, r.generate(010));
        assert_eq!(Cell::Empty, r.generate(011));
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
            "O_+++++++_O
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
            .replace(" ", "")
        );
    }

    #[test]
    fn test_field_from_string() {
        let s = "O_+++++++_O
             _#+#+#+#+#_
             spb++++++++
             +#+#+#+#+#+
             ++++B+B++++
             +#+#+#+#+#+
             ++++B++++++
             +#+#W#+#+#+
             ++++F++++++
             _#+#F#+#+#_
             O_++F+T++_O
            "
        .replace(" ", "");
        assert_eq!(Field::from_string_grid(&s).unwrap().string_grid(), s);
    }

    #[test]
    fn test_generated_with_start_points() {
        let field = Field::new(17, 13);
        println!("{}", field.string_grid());
        assert_eq!(
            field.start_positions(),
            vec![
                CellPosition { x: 0, y: 0 },
                CellPosition { x: 16, y: 0 },
                CellPosition { x: 0, y: 12 },
                CellPosition { x: 16, y: 12 }
            ]
        );
    }
}
