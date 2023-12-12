use crate::utils::Idx;
use std::fmt;
use std::ops::Index;
use std::ops::IndexMut;
use std::rc::Rc;

pub const TICKS_PER_SECOND: u32 = 60;

type HResult<T> = Result<T, String>;

/// A Time Stamp (not a duration)
#[derive(Default, Copy, Clone, PartialEq, PartialOrd)]
pub struct TimeStamp {
    inner: u32,
}

impl TimeStamp {
    pub fn ticks_from_start(self) -> u32 {
        self.inner
    }
}

impl fmt::Debug for TimeStamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "⌚{}", self.inner)
    }
}

impl std::ops::Add<Duration> for TimeStamp {
    type Output = Self;

    fn add(self, rhs: Duration) -> Self::Output {
        Self {
            inner: self.inner + rhs.ticks,
        }
    }
}

/// A Duration
#[derive(Copy, Clone, PartialEq, PartialOrd)]
pub struct Duration {
    ticks: u32,
}

impl Duration {
    pub fn from_ticks(ticks: u32) -> Self {
        Self { ticks }
    }

    pub fn from_seconds(seconds: f32) -> Self {
        Self {
            ticks: (seconds * TICKS_PER_SECOND as f32) as u32,
        }
    }
    pub fn ticks(self) -> u32 {
        self.ticks
    }
}

impl fmt::Debug for Duration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "⏳{}", self.ticks)
    }
}

#[derive(Copy, Clone, PartialEq)]
pub struct PlayerId(usize);

impl fmt::Debug for PlayerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Player{}", self.0)
    }
}

#[derive(Copy, Clone, PartialEq)]
pub enum Direction {
    North,
    West,
    South,
    East,
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
    pub x: u32,
    pub y: u32,
}

impl CellPosition {
    fn new(x: u32, y: u32) -> Self {
        Self { x, y }
    }

    /// Distance if in line of fire
    fn fire_distance(self, other: Self) -> Option<u32> {
        if self.x == other.x {
            Some(self.y.abs_diff(other.y))
        } else if self.y == other.y {
            Some(self.x.abs_diff(other.x))
        } else {
            None
        }
    }
}

impl fmt::Debug for CellPosition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({:02}/{:02})", self.x, self.y)
    }
}

/// Player positions
#[derive(Copy, Clone, PartialEq)]
pub struct Position {
    pub x: u32,
    pub y: u32,
}

impl fmt::Debug for Position {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{:02}/{:02}]", self.x, self.y)
    }
}

impl Position {
    /// Player position is tracked in this many fractions of a cell
    pub const PLAYER_POSITION_ACCURACY: u32 = 100;

    fn new(x: u32, y: u32) -> Self {
        Self { x, y }
    }

    /// move position `distance` into  `direction`
    fn add(self, direction: Direction, distance: i32) -> Option<Self> {
        let Self { x, y } = self;
        let (x, y) = match direction {
            Direction::North => (x, y.checked_add_signed(-distance)?),
            Direction::West => (x.checked_add_signed(-distance)?, y),
            Direction::South => (x, y.checked_add_signed(distance)?),
            Direction::East => (x.checked_add_signed(distance)?, y),
        };
        Some(Self::new(x, y))
    }

    fn as_cell_pos(self) -> CellPosition {
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
    wall: u8,
    clear: u8,
}

impl Default for Ratios {
    fn default() -> Self {
        Self::new(8, 9, 7, 1, 1, 20)
    }
}

impl Ratios {
    pub fn new(power: u8, speed: u8, bombs: u8, teleport: u8, wall: u8, clear: u8) -> Self {
        Self {
            power,
            speed,
            bombs,
            teleport,
            wall,
            clear,
        }
    }

    fn generate(&self, random: u32) -> Cell {
        let sum: u8 = self.power + self.speed + self.bombs + self.teleport + self.wall + self.clear;

        let mut random: u8 = (random % (u32::from(sum)))
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

        if random < self.wall {
            return Cell::Wall;
        }
        random -= self.wall;

        assert!(random < self.clear);
        Cell::Empty
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
    pub bomb_time: Duration,

    /// player walking speed at initial upgrade [cells/100/s]
    pub speed_multiplyer: u32,

    /// player walking speed upgrade start value [cells/100/s]
    pub speed_offset: u32,

    /// percentage that walking on bomb succeeds each update
    pub bomb_walking_chance: u8,

    /// percentage that walking on tombstone succeeds each update
    pub tombstone_walking_chance: u8,

    /// Power of Upgrade Paackets exploding
    pub upgrade_explosion_power: u8,

    /// how long before burning wood turns into something
    pub wood_burn_time: Duration,

    /// how long fire burns
    pub fire_burn_time: Duration,
}

impl Default for Rules {
    fn default() -> Self {
        Rules {
            width: 17,
            height: 13,
            players: 4,
            ratios: Ratios::default(),
            bomb_offset: 35,
            bomb_time: Duration::from_seconds(2.0),
            speed_multiplyer: 130,
            speed_offset: 700,
            bomb_walking_chance: 80,
            tombstone_walking_chance: 40,
            upgrade_explosion_power: 1,
            wood_burn_time: Duration::from_seconds(1.0),
            fire_burn_time: Duration::from_seconds(0.5),
        }
    }
}

impl Rules {
    /// Walking Speed based on `speed_powerup`
    /// returned speed is returned in `(Cell×TICKS_PER_SECOND)/(PLAYER_POSITION_ACCURACY×s)`
    /// so a speed of 1 is 60/100 cells/s
    ///
    /// Speed of input variables is Cells/100s
    fn get_update_walk_distance(&self, player_speed: u8) -> u32 {
        (self.speed_offset + (u32::from(player_speed) * self.speed_multiplyer)) * TICKS_PER_SECOND
            / Position::PLAYER_POSITION_ACCURACY
            / 100
    }
}

fn random(time: TimeStamp, r1: u32, r2: u32) -> u32 {
    let mut x: u32 = 42;
    for i in [time.ticks_from_start(), r1, r2] {
        for b in i.to_le_bytes() {
            x = x.overflowing_add(b.into()).0.overflowing_mul(31).0;
        }
    }
    x
}

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

#[derive(Debug, Clone)]
pub struct Player {
    /// Name the player chose
    name: String,

    /// Id of the player in the game
    id: PlayerId,

    /// Re-/Spawn place
    start_position: Position,
}

impl Player {
    pub fn new(name: String, id: PlayerId, start_position: Position) -> Self {
        Self {
            name,
            id,
            start_position,
        }
    }
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

    /// currently walking or placing?
    pub action: Action,
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
            bombs: 1,
            current_bombs_placed: 0,
            action: Action::idle(),
        }
    }

    fn move_(&mut self, position: Position) {
        self.position = position;
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
        self.power = u8::max(1, self.power / 2);
        self.speed = u8::max(1, self.speed / 2);
        self.bombs = u8::max(1, self.bombs / 2);
        self.position = start_position;
        self.action = Action::idle();
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
        expire: TimeStamp,
    },
    Fire {
        owner: PlayerId,
        expire: TimeStamp,
    },
    TombStone(PlayerId),
    Upgrade(Upgrade),
    Teleport,
    StartPoint,
    Wall,
    Wood,
    WoodBurning {
        expire: TimeStamp,
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

    pub fn from_char(chr: char) -> HResult<Self> {
        let owner = PlayerId(0);
        let power = 3;
        let expire = TimeStamp::default(); // everything expires on 1st tick

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
}

#[derive(Debug, Clone, PartialEq)]
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

    fn new_from_rules(rules: &Rules) -> Self {
        Self::new(rules.width, rules.height)
    }

    pub fn is_cell_in_field(&self, cell: CellPosition) -> bool {
        cell.x < self.width && cell.y < self.height
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

    pub fn new_from_string_grid(string: &str) -> HResult<Self> {
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
        (0..self.width).flat_map(move |x| (0..height).map(move |y| CellPosition::new(x, y)))
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

/// Constants of an active Game
#[derive(Debug)]
pub struct Game {
    pub name: String,
    pub players: Vec<Player>,
    pub rules: Rules,
    pub local_player: PlayerId,
}

impl Game {
    pub fn new_local_game(name: String, rules: Rules) -> Self {
        let field = Field::new(rules.width, rules.height);
        let start_positions = field.start_positions();

        assert!(start_positions.len() <= rules.players as _);

        let local_player = PlayerId(1);

        let players: Vec<Player> = (0..(rules.players as usize))
            .map(|id| Player {
                name: {
                    if id == local_player.0 as _ {
                        format!("Player {id}")
                    } else {
                        "Local Player".into()
                    }
                },
                id: PlayerId(id as _),
                start_position: Position::from_cell_position(start_positions[id]),
            })
            .collect();

        Self {
            name,
            players,
            rules,
            local_player,
        }
    }
}

#[derive(PartialEq, Clone)]
pub struct Action {
    pub walking: Option<Direction>,
    pub placing: bool,
}

impl Action {
    fn idle() -> Self {
        Self {
            walking: None,
            placing: false,
        }
    }
}

impl fmt::Debug for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.walking {
            Some(direction) => write!(f, "Waliking {direction:?}")?,
            None => write!(f, "Standing")?,
        }
        if self.placing {
            write!(f, " & placing")?
        }
        Ok(())
    }
}

/// The variable state of the game at a given time
#[derive(Debug, Clone)]
pub struct State {
    pub time: TimeStamp,
    pub field: Field,
    pub player_states: Vec<PlayerState>,
    pub game: Rc<Game>,
}

/// APIs
impl State {
    pub fn new(game: Rc<Game>) -> Self {
        let time = TimeStamp::default();

        let player_states: Vec<PlayerState> = game
            .players
            .iter()
            .map(|p| PlayerState::new(p.start_position))
            .collect();

        let field = Field::new_from_rules(&game.rules);

        Self {
            time,
            field,
            player_states,
            game,
        }
    }

    pub fn update(&mut self) {
        for i in 0..self.player_states.len() {
            // GAME_RULE: players with lower ID are processed earlier and win,
            // if both place bombs at the same spot 😎
            self.update_player(PlayerId(i));
        }
        self.update_field();
        self.increment_game_time();
    }

    pub fn set_player_action(&mut self, player_id: PlayerId, action: Action) {
        let player_state = &mut self.player_states[player_id.0];

        if player_state.action != action {
            log::debug!("{:?} {:?}.action := {:?}", self.time, player_id, action);
        }
        player_state.action = action;
    }
}

/// Update functions, that modify the Game State
impl State {
    fn increment_game_time(&mut self) {
        self.time = self.time + Duration::from_ticks(1);
    }

    /// advance a player 1 tick
    fn update_player(&mut self, player_id: PlayerId) {
        let action = self.player_states[player_id.0].action.clone();
        if action.placing {
            self.place_bomb(player_id);
        }
        if action.walking.is_some() {
            self.walk(player_id);
        };
    }

    fn walk(&mut self, player_id: PlayerId) {
        let player = &self.game.players[player_id.0];
        let player_state = &mut self.player_states[player_id.0];
        let position = player_state.position.add(
            player_state
                .action
                .walking
                .expect("only call walking if player is walking"),
            self.game
                .rules
                .get_update_walk_distance(player_state.speed)
                .try_into()
                .expect("walked distance fits i32"),
        );
        if let Some(position) = position {
            let cell_position = position.as_cell_pos();
            if self.field.is_cell_in_field(cell_position) {
                let cell = &self.field[cell_position];
                log::debug!(
                    "{:?} {:?} @ {:?} walking to {:?} == {:?} ({:?}) ",
                    self.time,
                    player_id,
                    player_state.position,
                    position,
                    cell_position,
                    &cell
                );
                match *cell {
                    Cell::StartPoint | Cell::Empty => {
                        player_state.move_(position);
                    }
                    Cell::Bomb { .. } => {
                        if random(self.time, position.x, position.y) % 100
                            < self.game.rules.bomb_walking_chance.into()
                        {
                            // GAME_RULE: walking on bombs randomly happens or doesn't, decided
                            // each update.
                            player_state.move_(position);
                        }
                    }
                    Cell::TombStone { .. } => {
                        if random(self.time, position.x, position.y) % 100
                            < self.game.rules.tombstone_walking_chance.into()
                        {
                            // GAME_RULE: walking on tombstones randomly happens or doesn't, decided
                            // each update.
                            player_state.move_(position);
                        }
                    }
                    Cell::Fire { owner, .. } => {
                        // GAME_RULE: walking into fire counts as kill by fire owner
                        // TODO: seperate counter?
                        player_state.die(owner, player.start_position);
                        self.player_states[owner.0].score(player_id);
                        self.field[cell_position] = Cell::TombStone(player_id);

                        log::info!("{:?} {:?} @ {:?} suicided", self.time, player_id, position,);
                    }
                    Cell::Upgrade(upgrade) => {
                        player_state.move_(position);
                        player_state.eat(upgrade);
                        self.field[cell_position] = Cell::Empty;

                        log::info!(
                            "{:?} {:?} @ {:?} ate {:?}, {:?}",
                            self.time,
                            player_id,
                            player_state.position,
                            upgrade,
                            player_state
                        );
                    }
                    Cell::Teleport => {
                        let targets: Vec<(CellPosition, &Cell)> = self
                            .field
                            .iter()
                            .filter(|&(target_position, target_cell)| {
                                *target_cell == Cell::Teleport && target_position != cell_position
                            })
                            .collect();
                        if targets.is_empty() {
                            log::info!(
                                "{:?} {:?} @ {:?} can not walk onto Teleport, it is not connected",
                                self.time,
                                player_id,
                                cell_position,
                            );
                            // GAME_RULE: you can not walk onto an unconnected TP :P
                            // player_state.move_(position);
                        } else {
                            let target = targets[random(self.time, position.x, position.y)
                                as usize
                                % targets.len()];
                            let (to, target_cell): (_, &Cell) = target;
                            assert_eq!(*target_cell, Cell::Teleport);

                            player_state.move_(Position::from_cell_position(to));

                            debug_assert_eq!(self.field[cell_position], Cell::Teleport);
                            debug_assert_eq!(self.field[to], Cell::Teleport);
                            self.field[cell_position] = Cell::Empty;
                            self.field[to] = Cell::Empty;
                            log::info!(
                                "{:?} {:?} @ {:?} ported to {:?}",
                                self.time,
                                player_id,
                                cell_position,
                                to
                            );
                        }
                    }
                    Cell::Wall | Cell::Wood | Cell::WoodBurning { .. } => {} /* no walking through walls */
                }
            }
        }
    }

    fn place_bomb(&mut self, player_id: PlayerId) {
        let player_state = &mut self.player_states[player_id.0];
        // GAME RULE: can not place more bombs than you have bomb powerups
        if player_state.current_bombs_placed >= player_state.bombs {
            log::info!(
                "{:?} {:?} out of bombs {:?}",
                self.time,
                player_id,
                player_state.bombs
            );
        } else {
            let position = match player_state.action.walking {
                Some(direction) => player_state
                    .position
                    .add(direction, -self.game.rules.bomb_offset)
                    .unwrap_or(player_state.position),
                None => player_state.position,
            };

            let cell_position = position.as_cell_pos();
            if self.field.is_cell_in_field(cell_position) {
                let cell = &mut self.field[cell_position];

                // GAME_RULE: placing a bomb onto a powerup gives you that powerup AFTER checking
                // if you have enough bombs to place, but BEFORE placing the bomb (bomb count
                // is not considered, power is)
                if let Cell::Upgrade(upgrade) = *cell {
                    log::info!(
                        "{:?} {:?} @ {:?}: ate {:?} while placing",
                        self.time,
                        player_id,
                        player_state.position,
                        upgrade,
                    );
                    player_state.eat(upgrade);
                }

                // TODO: placing Bombs into TP and have the Bomb Port would be funny
                // TODO: place Bomb into fire for immediate explosion?

                // GAME_RULE: Bombs can only be placed on empty Cells (after eating any powerups
                // there were)
                if Cell::Empty == *cell {
                    player_state.current_bombs_placed += 1;
                    *cell = Cell::Bomb {
                        owner: player_id,
                        expire: self.time + self.game.rules.bomb_time,
                        // GAME_RULE: power is set AFTER eating powerups at cell
                        power: player_state.power,
                    };
                    log::info!(
                        "{:?} {:?} @ {:?} placed  {:?}",
                        self.time,
                        player_id,
                        player_state.position,
                        cell
                    );
                }
            } else {
                log::debug!(
                    "{:?} {:?} @ {:?} not placing to {:?}",
                    self.time,
                    player_id,
                    player_state.position,
                    position
                );
                // TODO: log not placing at position (x or y too large)
            }
        }
    }

    /// set a cell on fire.
    ///
    /// `consider_tp` if target is a teleport, explode a random other teleport too.
    ///
    /// returns if the fire should continue further in that direction
    fn set_on_fire(&mut self, cell: CellPosition, owner: PlayerId, consider_tp: bool) -> bool {
        let (explodes, power, owner) = match self.field[cell] {
            // TODO: Tombstone Explodes based on players schinken?
            Cell::StartPoint | Cell::Fire { .. } | Cell::Empty | Cell::TombStone(..) => {
                (true, 0, owner)
            }
            Cell::Bomb {
                power,
                owner: bomb_owner,
                ..
            } => {
                log::info!("{cell:?}: destroying {owner:?}'s bomb");
                self.player_states[bomb_owner.0].current_bombs_placed -= 1;

                // GAME_RULE: owner of secondary Bomb takes the credit
                (true, power, bomb_owner)
            }
            Cell::Upgrade(upgrade) => {
                log::info!("{cell:?}: destroying {upgrade:?}");

                (true, self.game.rules.upgrade_explosion_power, owner)
            }
            Cell::Teleport => {
                let explodes = if consider_tp {
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
                    if ports.is_empty() {
                        log::info!("{cell:?}: destroying Teleport (no remote TP found)");
                        false
                    } else {
                        let other = ports[random(self.time, cell.x, cell.y).idx() % ports.len()];
                        log::info!("{cell:?}: destroying Teleport, tunneling to {other:?}");
                        self.set_on_fire(other, owner, false);
                        true
                    }
                } else {
                    true
                };
                (explodes, self.game.rules.upgrade_explosion_power, owner)
            }
            Cell::WoodBurning { .. } | Cell::Wall => (false, 0, owner),
            Cell::Wood => {
                let expire = self.time + self.game.rules.wood_burn_time;
                self.field[cell] = Cell::WoodBurning { expire };
                log::info!("{cell:?}: setting wall on fire until {expire:?}");
                (false, 0, owner)
            }
        };
        if explodes {
            self.field[cell] = Cell::Fire {
                owner,
                expire: self.time + self.game.rules.fire_burn_time,
            };
            for (id, p) in self.player_states.iter_mut().enumerate() {
                if p.position.as_cell_pos() == cell {
                    p.die(owner, self.game.players[id].start_position);
                    self.field[cell] = Cell::TombStone(PlayerId(id));
                }
            }

            let power: isize = power.into();
            if power > 0 {
                let x = cell.x as isize;
                let y = cell.y as isize;
                for (dx, dy) in [(-1, 0), (1, 0), (0, 1), (0, -1)] {
                    for i in 1..=power {
                        let x = x + dx * i;
                        let y = y + dy * i;
                        if x >= 0 && y >= 0 {
                            let pos = CellPosition::new(x as u32, y as u32);
                            if self.field.is_cell_in_field(pos)
                                && !self.set_on_fire(pos, owner, true)
                            {
                                break;
                            }
                        } else {
                            break;
                        }
                    }
                }
            }
        }
        explodes
    }

    fn update_field(&mut self) {
        for cell_idx in self.field.iter_indices() {
            let cell = &mut self.field[cell_idx];
            match *cell {
                Cell::Bomb { owner, expire, .. } => {
                    assert!(expire >= self.time);
                    if expire == self.time {
                        self.set_on_fire(cell_idx, owner, true);
                    }
                }
                Cell::Fire { expire, .. } => {
                    assert!(expire >= self.time);
                    if expire == self.time {
                        *cell = Cell::Empty;
                    }
                }
                Cell::WoodBurning { expire } => {
                    assert!(expire >= self.time);
                    if expire == self.time {
                        let r = random(self.time, cell_idx.x, cell_idx.y);
                        *cell = self.game.rules.ratios.generate(r);
                    }
                }

                Cell::TombStone(_)
                | Cell::Upgrade(_)
                | Cell::Teleport
                | Cell::StartPoint
                | Cell::Empty
                | Cell::Wall
                | Cell::Wood => {}
            }
        }
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

    //TODO #[test]
    fn test_walking_distance() {
        let r = Rules::default();
        assert_eq!(r.get_update_walk_distance(1), 4);
        assert_eq!(r.get_update_walk_distance(2), 4);
    }

    #[test]
    fn test_random() {
        let r = random(TimeStamp::default(), 0, 0);
        assert_eq!(r, random(TimeStamp::default(), 0, 0));
        assert!(r != random(TimeStamp::default() + Duration::from_ticks(1), 0, 0));
        assert!(r != random(TimeStamp::default(), 1, 0));
        assert!(r != random(TimeStamp::default(), 0, 1));
    }

    fn game() -> State {
        let player1 = Player::new("test player 1".to_owned(), PlayerId(0), Position::new(0, 0));
        let local_player = player1.id;
        let rules = Rules::default();
        let game = Game {
            name: "Test Game".to_owned(),
            players: vec![player1],
            rules,
            local_player,
        };

        let game = Rc::new(game);

        let mut gs = State::new(game);
        gs.player_states[0].current_bombs_placed = 42; // Hack, so bombs can explode without int
                                                       // underrun. If a test cares, it should set
                                                       // this correctly
        gs
    }

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
        assert_eq!(Cell::Wall, r.generate(008));
        assert_eq!(Cell::Wall, r.generate(009));
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
    fn test_static_cells_dont_explode() {
        let mut gs = game();

        let s = "_DspbTT0W+";
        let field = Field::new_from_string_grid(s).unwrap();
        gs.field = field.clone();

        let orig_gs = gs.clone();

        gs.update_field();

        assert_eq!(orig_gs.field, gs.field);
        assert_eq!(orig_gs.player_states, gs.player_states);
    }

    fn field_looks_equal(actual: &Field, expected: &str) -> bool {
        let expected = Field::new_from_string_grid(expected).expect("parseable");
        if actual.width != expected.width {
            println!("width different {} != {}", actual.width, expected.width);
            false
        } else if actual.height != expected.height {
            println!("height different {} != {}", actual.height, expected.height);
            false
        } else {
            let mut eq = true;
            for cell in actual.iter_indices() {
                if actual[cell].to_char() != expected[cell].to_char() {
                    println!(
                        " unexpected at {:?}: {:#?} != {:#?}",
                        cell, actual[cell], expected[cell]
                    );
                    eq = false;
                }
            }
            if !eq {
                println!(
                    "Expected:\n    {}",
                    actual.string_grid().replace('\n', "\n    ")
                );
                println!(
                    "Actual:\n    {}",
                    actual.string_grid().replace('\n', "\n    ")
                );
            }
            eq
        }
    }
    #[test]
    fn test_bomb_explodes_after_time() {
        let mut gs = game();
        let x = CellPosition::new(1, 1);
        gs.field[x] = Cell::Bomb {
            owner: PlayerId(0),
            power: 1,
            expire: gs.time + Duration::from_ticks(3),
        };
        gs.increment_game_time();
        gs.update_field();
        if let Cell::Bomb { .. } = gs.field[x] {
        } else {
            panic!();
        }
        gs.increment_game_time();
        gs.update_field();
        if let Cell::Bomb { .. } = gs.field[x] {
        } else {
            panic!();
        }
        gs.increment_game_time();
        gs.update_field();
        if let Cell::Fire { .. } = gs.field[x] {
            // pass
        } else {
            panic!();
        }
    }

    #[test]
    fn test_bomb_explodes() {
        let mut gs = game();
        gs.field = Field::new_from_string_grid(
            "
            _________
            _________
            _________
            _________
            ____B____
            _________
            _________
            _________
            _________
        ",
        )
        .unwrap();
        gs.update_field();

        let expected = "
            _________
            ____F____
            ____F____
            ____F____
            _FFFFFFF_
            ____F____
            ____F____
            ____F____
            _________
            ";
        assert!(field_looks_equal(&gs.field, expected));
    }

    #[test]
    fn test_bomb_explosion_counts_placed_bombs() {
        let mut gs = game();
        gs.field[CellPosition::new(1, 1)] = Cell::Bomb {
            owner: PlayerId(0),
            power: 1,
            expire: gs.time,
        };
        gs.player_states[0].current_bombs_placed = 42;
        gs.update_field();
        assert_eq!(gs.player_states[0].current_bombs_placed, 41);
    }
    #[test]
    fn test_walls_catch_fire() {
        let mut gs = game();

        gs.field = Field::new_from_string_grid(
            "
            ++++++++++
            ++_+++++++
            ++B___+++_
            ++_+++++++
            ++_+++++++
            ++++++++++
        ",
        )
        .unwrap();

        gs.update_field();

        let expected = "
            ++W+++++++
            ++F+++++++
            +WFFFF+++_
            ++F+++++++
            ++F+++++++
            ++W+++++++
            ";
        assert!(field_looks_equal(&gs.field, expected));
    }

    #[test]
    fn test_powerup_explodes() {
        let mut gs = game();

        gs.field = Field::new_from_string_grid(
            "
            __________
            __________
            __________
            b_________
            __________
            __________
            B_________
        ",
        )
        .unwrap();

        gs.update_field();

        let expected = "
            __________
            __________
            F_________
            FF________
            F_________
            F_________
            FFFF______
            ";
        assert!(field_looks_equal(&gs.field, expected));
    }
}
