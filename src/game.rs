use std::ops::Index;
use std::ops::IndexMut;

/// Player position is tracked in this many fractions of a cell
const PLAYER_POSITION_ACCURACY: u32 = 100;

#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
pub struct Time(u32);

impl std::ops::Add<Time> for Time {
    type Output = Time;

    fn add(self, rhs: Time) -> Self::Output {
        Time(self.0 + rhs.0)
    }
}

impl std::ops::Add<i32> for Time {
    type Output = Time;

    fn add(self, rhs: i32) -> Self::Output {
        Time((self.0 as i32 + rhs) as u32)
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

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct CellPosition {
    x: u32,
    y: u32,
}

impl CellPosition {
    fn new(x: u32, y: u32) -> Self {
        Self { x, y }
    }

    fn same_row_distance(&self, other: &Self) -> Option<u32> {
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
    x: u32,
    y: u32,
}

impl Position {
    fn new(x: u32, y: u32) -> Self {
        Self { x, y }
    }

    /// move position `distance` into  `direction
    fn add(&self, direction: Direction, distance: i32) -> Option<Self> {
        let Self { x, y } = *self;
        let x = x as i32;
        let y = y as i32;
        let (x, y) = match direction {
            Direction::North => (x, y - distance),
            Direction::West => (x - distance, y),
            Direction::South => (x, y + distance),
            Direction::East => (x + distance, y),
        };
        if x > 0 && y > 0 {
            Some(Self::new(x as u32, y as u32))
        } else {
            None
        }
    }

    fn as_cell_pos(&self) -> CellPosition {
        CellPosition {
            x: self.x / PLAYER_POSITION_ACCURACY,
            y: self.y / PLAYER_POSITION_ACCURACY,
        }
    }

    fn from_cell_position(p: CellPosition) -> Self {
        Self {
            x: p.x * PLAYER_POSITION_ACCURACY + PLAYER_POSITION_ACCURACY / 2,
            y: p.y * PLAYER_POSITION_ACCURACY + PLAYER_POSITION_ACCURACY / 2,
        }
    }
}

/// Ratios of Wood turning into those cell types:
#[derive(Debug)]
pub struct Ratios {
    power: u8,
    speed: u8,
    bombs: u8,
    teleport: u8,
    wood: u8,
    clear: u8,
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

    fn generate(&self, random: u32) -> Cell {
        let sum = self.power + self.speed + self.bombs + self.teleport + self.wood + self.clear;

        let mut random: u8 = (random % (sum as u32)).try_into().unwrap();

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

#[derive(Debug)]
pub struct Rules {
    /// Ratios what comes out of burned down walls
    ratios: Ratios,

    /// how far behind the player the bomb is placed [cell/100]
    bomb_offset: i32,

    /// time after bomb placement that the bomb explodes
    bomb_time: Time,

    /// Time after which a player is lagging and does not move forward
    lag_time: Time,

    /// player walking speed at initial upgrade [cells/100/s]
    speed_multiplyer: u32,

    /// player walking speed upgrade start value
    speed_offset: u32,

    /// percentage that walking on bomb succeeds each update
    bomb_walking_chance: u8,

    /// Power of Upgrade Paackets exploding
    upgrade_explosion_power: u8,

    /// how long before burning wood turns into something
    wood_burn_time: Time,

    /// how long fire burns
    fire_burn_time: Time,
}

impl Rules {
    fn get_update_walk_distance(&self, player_speed: u8) -> u32 {
        (self.speed_offset + u32::from(player_speed)) * self.speed_multiplyer
    }
}

fn random(time: Time, r1: u32, r2: u32) -> u32 {
    let mut x: u32 = 42;
    for i in [time.0, r1, r2] {
        for b in i.to_le_bytes() {
            x = x.overflowing_mul(31).0.overflowing_add(b as u32).0;
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

#[derive(Debug, Clone, PartialEq)]
struct PlayerState {}

#[derive(Debug, Clone)]
struct Player {
    /// Name the player chose
    name: String,

    /// Id of the player in the game
    id: PlayerId,

    /// Re-/Spawn place
    start_position: Position,

    /// current position
    position: Position,

    /// number of deaths since the game started
    deaths: u32,

    /// number of kills since the game started
    kills: u32,

    /// current bomb power upgrades
    power: u8,

    /// current walking speed upgrades
    speed: u8,

    /// current bomb capacity upgrades
    bombs: u8,

    /// current placed bombs. Increased when placing, decreased when exploding.
    current_bombs_placed: u8,

    /// current action
    action: Action,

    /// current direction
    direction: Direction,
    // TODO: track total walking distance, total bombs, ...
}

impl Player {
    fn eat(&mut self, upgrade: Upgrade) {
        let up = match upgrade {
            Upgrade::Speed => &mut self.speed,
            Upgrade::Power => &mut self.power,
            Upgrade::Bombs => &mut self.bombs,
        };
        *up = up.saturating_add(1);
    }

    fn die(&mut self, _killed_by: PlayerId) {
        self.power = 1;
        self.speed = 1;
        self.bombs = 1;
        self.position = self.start_position;
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

#[derive(Debug)]
struct Field {
    width: u32,
    height: u32,
    cells: Vec<Cell>,
}

impl Field {
    fn new(width: u32, height: u32) -> Self {
        let cells: Vec<Cell> = (0..width)
            .into_iter()
            .flat_map(|x| {
                (0..height).into_iter().map(move |y| {
                    let x = if x >= width / 2 { width - x - 1 } else { x };
                    let y = if y >= height / 2 { height - y - 1 } else { y };

                    if x + y == 0 {
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

    fn is_cell_in_field(&self, cell: CellPosition) -> bool {
        if cell.x >= self.width {
            false
        } else if cell.y >= self.height {
            false
        } else {
            true
        }
    }

    fn string_grid(&self) -> String {
        let mut s = String::new();
        for y in 0..self.height {
            for x in 0..self.width {
                let cell = &self[CellPosition::new(x, y)];
                // TODO: how about these: _💣💥🪦🏃💪🧨🚪🏳🧱🪜🔥

                let chr = match cell {
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
                    Cell::WoodBurning(_) => '+',
                };
                s.push(chr);
            }
            s.push('\n');
        }
        s
    }

    fn iter<'f>(&'f self) -> FieldIterator<'f> {
        FieldIterator::new(self)
    }

    fn iter_mut<'f>(&'f mut self) -> FieldMutIterator<'f> {
        FieldMutIterator::new(self)
    }
}

impl Index<CellPosition> for Field {
    type Output = Cell;

    fn index(&self, index: CellPosition) -> &Self::Output {
        if self.is_cell_in_field(index) {
            &self.cells[(index.y * self.width + index.x) as usize]
        } else {
            panic!("y > height: {} > {}", index.y, self.height)
        }
    }
}

impl IndexMut<CellPosition> for Field {
    fn index_mut(&mut self, index: CellPosition) -> &mut Self::Output {
        if self.is_cell_in_field(index) {
            &mut self.cells[(index.y * self.width + index.x) as usize]
        } else {
            panic!("y > height: {} > {}", index.y, self.height)
        }
    }
}

struct FieldIterator<'f> {
    field: &'f Field,
    pos: CellPosition,
}
impl<'f> FieldIterator<'f> {
    fn new(field: &'f Field) -> Self {
        Self {
            field,
            pos: CellPosition::new(0, 0),
        }
    }
}
impl<'f> Iterator for FieldIterator<'f> {
    type Item = (CellPosition, &'f Cell);

    fn next(&mut self) -> Option<Self::Item> {
        self.pos.x += 1;
        if self.pos.x >= self.field.width {
            self.pos.x = 0;
            self.pos.y += 1;
        }
        if self.pos.y >= self.field.height {
            None
        } else {
            Some((self.pos, &self.field[self.pos]))
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

impl<'f> Iterator for FieldMutIterator<'f> {
    type Item = (CellPosition, &'f mut Cell);

    fn next(&mut self) -> Option<Self::Item> {
        self.pos.x += 1;
        if self.pos.x >= self.field.width {
            self.pos.x = 0;
            self.pos.y += 1;
        }
        if self.pos.y >= self.field.height {
            None
        } else {
            Some((self.pos, &mut self.field[self.pos]))
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
pub struct Game {
    name: String,
    players: Vec<Player>,
    time: Time,
    field: Field,
    rules: Rules,
}
impl Game {
    /// advance a player 1 tick and generate the events from that
    fn player_update_event(&self, player_id: PlayerId) -> Vec<Event> {
        let mut events: Vec<Event> = Vec::new();
        let player = &self.players[player_id.0];

        match player.action {
            Action::Standing | Action::Placing => { /*nothing to do */ }
            Action::Walking => {
                let position = player.position.add(
                    player.direction,
                    self.rules.get_update_walk_distance(player.speed) as i32,
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
                                    < self.rules.bomb_walking_chance.into()
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
                                    let target = targets[random(self.time, position.x, position.y)
                                        as usize
                                        % targets.len()];
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
                    self.players[player.0].position = *position;
                }
                Event::Eat(player, upgrade, position) => {
                    self.players[player.0].eat(*upgrade);
                    self.field[*position] = Cell::Empty;
                }
                Event::Killed { dead, owner, at } => {
                    self.players[dead.0].die(*owner);
                    self.field[*at] = Cell::TombStone;
                    self.players[owner.0].score(*dead);
                }
                Event::Place {
                    player_id,
                    cell,
                    expire,
                    power,
                } => {
                    let player = &mut self.players[player_id.0];
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
                    let player = &mut self.players[player_id.0];
                    player.position = Position::from_cell_position(*to);

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
                    let player = &mut self.players[player_id.0];
                    player.action = *action;
                    player.direction = *direction;
                }
                Event::CellUpdate(pos, cell) => {
                    self.field[*pos] = *cell;
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
            Cell::Upgrade(_) => (true, self.rules.upgrade_explosion_power),
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
                        let other = ports[random(self.time, cell.x, cell.y) as usize % ports.len()];
                        self.set_on_fire(other, owner, false);
                    }
                }
                (true, self.rules.upgrade_explosion_power)
            }
            Cell::StartPoint | Cell::WoodBurning(_) | Cell::Wall => (false, 0),
            Cell::Wood => {
                self.field[cell] = Cell::WoodBurning(self.rules.wood_burn_time);
                (false, 0)
            }
        };
        if burns {
            self.field[cell] = Cell::Fire {
                owner,
                expire: self.rules.fire_burn_time,
            };
            if power > 0 {
                for i in 1..=(power as u32) {
                    self.set_on_fire(CellPosition::new(cell.x + i, cell.y), owner, true) || break;
                }
                for i in 1..=(power as u32) {
                    self.set_on_fire(CellPosition::new(cell.x - i, cell.y), owner, true) || break;
                }
                for i in 1..=(power as u32) {
                    self.set_on_fire(CellPosition::new(cell.x, cell.y + i), owner, true) || break;
                }
                for i in 1..=(power as u32) {
                    self.set_on_fire(CellPosition::new(cell.x, cell.y - i), owner, true) || break;
                }
            }
        }
        burns
    }

    fn field_update_events(&mut self) {
        for (cell_idx, cell) in self.field.iter_mut() {
            match cell {
                Cell::Bomb {
                    owner,
                    power,
                    expire,
                } => {
                    expire.0 -= 1;
                    if expire.0 == 0 {
                        self.set_on_fire(CellPosition::new(cell_idx.x, cell_idx.y), *owner, true);
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
                    *cell = self.rules.ratios.generate(r);
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
        for i in 0..self.players.len() {
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
        let player = &self.players[player_id.0];

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
                            .add(player.direction, -self.rules.bomb_offset);

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
                                        expire: self.time + self.rules.bomb_time,
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
        let r = random(Time(0), Position::new(0, 0));
        assert_eq!(r, random(Time(0), Position::new(0, 0)));
        assert!(r != random(Time(1), Position::new(0, 0)));
        assert!(r != random(Time(0), Position::new(1, 0)));
        assert!(r != random(Time(0), Position::new(0, 1)));
        assert!(r != random(Time(2), Position::new(0, 0)));
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
        let field = Field::new(11, 11);

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
             _#+#+#+#+#_
             O_+++++++_O
            "
            .replace(" ", "")
        );
    }
}
