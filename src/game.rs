use std::ops::Index;
use std::ops::IndexMut;

/// Player position is tracked in this many fractions of a cell
const PLAYER_POSITION_ACCURACY: i32 = 100;

#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
pub struct Time(u32);

impl std::ops::Add<Time> for Time {
    type Output = Time;

    fn add(self, rhs: Time) -> Self::Output {
        Time(self.0 + rhs.0)
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct PlayerId(usize);

#[derive(Debug, Copy, Clone, PartialEq)]
enum Direction {
    North,
    West,
    South,
    East,
}

/// Player positions
#[derive(Debug, Copy, Clone, PartialEq)]
struct Position {
    x: i32,
    y: i32,
}

impl Position {
    fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    /// move position `distance` into  `direction
    fn add(&self, direction: Direction, distance: i32) -> Self {
        let Self { x, y } = *self;
        let (x, y) = match direction {
            Direction::North => (x, y - distance),
            Direction::West => (x - distance, y),
            Direction::South => (x, y + distance),
            Direction::East => (x + distance, y),
        };
        Self { x, y }
    }
}

/// Ratios of Wood turning into those cell types:
#[derive(Debug)]
pub struct Ratios {
    power: u8,
    speed: u8,
    bombs: u8,
    schinken: u8,
    teleport: u8,
    wood: u8,
    clear: u8,
}

impl Ratios {
    pub fn new(
        power: u8,
        speed: u8,
        bombs: u8,
        schinken: u8,
        teleport: u8,
        wood: u8,
        clear: u8,
    ) -> Self {
        Self {
            power,
            speed,
            bombs,
            schinken,
            teleport,
            wood,
            clear,
        }
    }

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

        if random < self.schinken {
            return Cell::Upgrade(Upgrade::Schinken);
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
    speed_multiplyer: i32,

    /// player walking speed upgrade start value
    speed_offset: i32,

    /// percentage that walking on bomb succeeds each update
    bomb_walking_chance: u8,
}

impl Rules {
    fn get_update_walk_distance(&self, player_speed: u8) -> i32 {
        (i32::from(player_speed) + self.speed_offset) * self.speed_multiplyer
    }
}

fn random(time: Time, r1: i32, r2: i32) -> u32 {
    let mut x: u32 = 42;
    for i in [time.0 as i32, r1, r2] {
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
    Schinken,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Action {
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
    /// Name the player chose
    name: String,

    /// Id of the player in the game
    id: PlayerId,

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

    /// current schinken?
    schinken: u8,

    /// current placed bombs. Increased when placing, decreased when exploding.
    current_bombs_placed: u8,

    /// Bombs placed since game start
    total_bombs_placed: u32,

    /// distance walked since game start
    distance_walked: u32,

    // current State
    state: PlayerState,
}

impl Player {
    fn new(name: String, id: PlayerId) -> Self {
        Self {
            name,
            id,
            position: Position { x: 0, y: 0 },
            deaths: 0,
            kills: 0,
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

    fn eat(&mut self, upgrade: Upgrade) {
        let up = match upgrade {
            Upgrade::Speed => &mut self.speed,
            Upgrade::Power => &mut self.power,
            Upgrade::Bombs => &mut self.bombs,
            Upgrade::Schinken => &mut self.schinken,
        };
        *up = up.saturating_add(1);
    }

    fn die(&mut self, position: Position, _killed_by: PlayerId, time: Time) {
        self.power = 1;
        self.speed = 1;
        self.bombs = 1;
        self.position = position;
        self.current_bombs_placed = 0;
        self.state.since = time;
        self.state.action = Action::Standing;
        self.state.direction = Direction::South;
    }

    fn score(&mut self, _killed: PlayerId) {
        self.kills += 1;
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub enum Cell {
    #[default]
    Clear,
    Bomb {
        owner: PlayerId,
        expire: Time,
    },
    Fire {
        owner: PlayerId,
        expire: Time,
    },
    HansDead,
    HansGore,
    Upgrade(Upgrade),
    Teleport,
    StartPoint,
    Wall,
    Wood,
    WoodBurning(Time),
}

#[derive(Debug, Clone)]
pub struct PlayerAction {
    time: Time,
    player: PlayerId,
    action: Action,
    direction: Direction,
}

#[derive(Debug)]
pub enum GameError {
    InvalidPlayerId(PlayerAction),
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

    fn position_to_cell_index(&self, position: Position) -> Option<(usize, usize)> {
        if position.x < 0 {
            return None;
        }
        if position.y < 0 {
            return None;
        }
        let x = position.x / PLAYER_POSITION_ACCURACY;
        let y = position.y / PLAYER_POSITION_ACCURACY;
        let (x, y) = (x as usize, y as usize);
        if x >= self.width {
            return None;
        }
        if y >= self.height {
            return None;
        }
        Some((x, y))
    }

    fn cell_index_to_position(x: usize, y: usize) -> Position {
        Position {
            x: x as i32 * PLAYER_POSITION_ACCURACY + PLAYER_POSITION_ACCURACY / 2,
            y: y as i32 * PLAYER_POSITION_ACCURACY + PLAYER_POSITION_ACCURACY / 2,
        }
    }

    fn string_grid(&self) -> String {
        let mut s = String::new();
        for y in 0..self.height {
            for x in 0..self.width {
                let chr = match self[(x, y)] {
                    Cell::Clear => '_',
                    Cell::Bomb { .. } => 'B',
                    Cell::Fire { .. } => 'F',
                    Cell::HansDead => 'D',
                    Cell::HansGore => 'd',
                    Cell::Upgrade(pu) => match pu {
                        Upgrade::Speed => 'S',
                        Upgrade::Power => 'P',
                        Upgrade::Bombs => 'b',
                        Upgrade::Schinken => 'h',
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
        FieldIterator {
            field: self,
            x: 0,
            y: 0,
        }
    }
}

impl Index<(usize, usize)> for Field {
    type Output = Cell;

    fn index(&self, index: (usize, usize)) -> &Self::Output {
        let (x, y) = index;
        if x > self.width {
            panic!("x > width: {} > {}", x, self.width);
        }
        if y > self.height {
            panic!("y > height: {} > {}", y, self.height);
        }
        &self.cells[y * self.width + x]
    }
}

impl IndexMut<(usize, usize)> for Field {
    fn index_mut(&mut self, index: (usize, usize)) -> &mut Self::Output {
        let (x, y) = index;
        if x > self.width {
            panic!("x > width: {} > {}", x, self.width);
        }
        if y > self.height {
            panic!("y > height: {} > {}", y, self.height);
        }
        &mut self.cells[y * self.width + x]
    }
}

struct FieldIterator<'f> {
    field: &'f Field,
    x: usize,
    y: usize,
}

impl<'f> Iterator for FieldIterator<'f> {
    type Item = ((usize, usize), &'f Cell);

    fn next(&mut self) -> Option<Self::Item> {
        self.x += 1;
        if self.x >= self.field.width {
            self.x = 0;
            self.y += 1;
        }
        if self.y >= self.field.height {
            None
        } else {
            Some(((self.x, self.y), &self.field[(self.x, self.y)]))
        }
    }
}

#[derive(Debug)]
pub struct Game {
    name: String,
    players: Vec<Player>,
    time: Time,
    field: Field,
    rules: Rules,
}

pub struct Update {
    time: Time,
    player: PlayerId,
    events: Vec<Event>,
}
enum Event {
    Move { position: Position },
    Eat { upgrade: Upgrade },
    Killed { bomb_owner: PlayerId },
    Place { position: Position },
    Teleport { x: usize, y: usize },
}

impl Game {
    fn update_player(&mut self, player_id: PlayerId) {
        let player = &mut self.players[player_id.0];

        match player.state.action {
            Action::Standing | Action::Placing => { /*nothing to do */ }
            Action::Walking => {
                let new_pos = player.position.add(
                    player.state.direction,
                    self.rules.get_update_walk_distance(player.speed),
                );

                let player_cell_index = match self.field.position_to_cell_index(new_pos) {
                    None => return,
                    Some(xy) => xy,
                };

                let cell = &self.field[player_cell_index];
                match cell {
                    Cell::StartPoint | Cell::HansGore | Cell::Clear => {
                        player.position = new_pos;
                    }
                    Cell::Bomb { .. } | Cell::HansDead => {
                        if random(self.time, new_pos.x, new_pos.y) % 100
                            < self.rules.bomb_walking_chance.into()
                        {
                            player.position = new_pos;
                        }
                    }
                    Cell::Fire { owner, .. } => {
                        let start_point = self
                            .field
                            .iter()
                            .filter(|(_, cell)| **cell == Cell::StartPoint)
                            .nth(player.id.0)
                            .expect("Player's StartPoint still exists");
                        let ((x, y), _cell) = start_point;
                        player.die(Field::cell_index_to_position(x, y), *owner, self.time);
                        let id = player.id;
                        self.players[owner.0].score(id)
                    }
                    Cell::Upgrade(upgrade) => {
                        player.position = new_pos;
                        player.eat(*upgrade);
                    }
                    Cell::Teleport => {
                        let targets: Vec<((usize, usize), &Cell)> = self
                            .field
                            .iter()
                            .filter(|(xy, cell)| **cell == Cell::Clear && *xy != player_cell_index)
                            .collect();
                        if targets.len() > 1 {
                            let target = targets
                                [random(self.time, new_pos.x, new_pos.y) as usize % targets.len()];
                            let ((x, y), _cell): (_, &Cell) = target;
                            player.position = Field::cell_index_to_position(x, y)
                        } else {
                            player.position = new_pos;
                        }
                    }
                    Cell::Wall | Cell::Wood | Cell::WoodBurning(_) => {} /* no walking through walls */
                }
            }
        };
    }

    pub fn update(&mut self) {
        self.time.0 += 1;
        for i in 0..self.players.len() {
            self.update_player(PlayerId(i));
        }
    }

    pub fn player_action(&mut self, player_action: PlayerAction) -> Result<bool, GameError> {
        let player = self
            .players
            .get_mut(player_action.player.0)
            .ok_or_else(|| GameError::InvalidPlayerId(player_action.clone()))?;

        // TODO: backtrack
        if player.state.action != player_action.action
            || player.state.direction != player_action.direction
        {
            player.state = PlayerState {
                since: player_action.time,
                action: player_action.action,
                direction: player_action.direction,
            };
            match player_action.action {
                Action::Standing => Ok(true),
                Action::Walking => Ok(true),
                Action::Placing => {
                    // TODO Self::player_place_bomb(player, &mut self.field, &self.)
                    if player.current_bombs_placed >= player.bombs {
                        // GAME RULE: can not place more bombs than you have bomb powerups
                        return Ok(false);
                    }

                    let cell = self.field.position_to_cell_index(
                        player
                            .position
                            .add(player.state.direction, -self.rules.bomb_offset),
                    );
                    let cell = match cell {
                        Some(cell) => cell,
                        None => {
                            // GAME_RULE: placing a Bomb outside of the field is NoOp
                            return Ok(false);
                        }
                    };
                    let cell = &mut self.field[cell];

                    // GAME_RULE: placing a bomb onto a powerup gives you that powerup AFTER checking
                    // if you have enough bombs to place
                    if let Cell::Upgrade(upgrage) = cell {
                        player.eat(*upgrage);
                        *cell = Cell::Clear;
                    }
                    // TODO: placing Bombs into TP and have the Bomb Port would be funny
                    // TODO: place Bomb into fire for immediate explosion?
                    // GAME_RULE: Bombs can only be placed on empty Cells (after eating any powerups
                    // there were)
                    if Cell::Clear != *cell {
                        // GAME_RULE: placing a Bomb on Cells not PowerUp or Clear is NoOp.
                        Ok(false)
                    } else {
                        player.total_bombs_placed += 1;
                        player.current_bombs_placed += 1;
                        // GAME_RULE: bomb is owned by the one who placed it
                        // GAME_RULE: bom has fixed timeout (TODO: add randomness?)
                        *cell = Cell::Bomb {
                            owner: player.id,
                            expire: player_action.time + self.rules.bomb_time,
                        };

                        Ok(true)
                    }
                }
            }
        } else {
            Ok(false)
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_player_coord_add() {
        let p = Position { x: 100, y: 100 };

        assert_eq!(p.add(Direction::North, 10), Position { x: 100, y: 90 });
        assert_eq!(p.add(Direction::South, 10), Position { x: 100, y: 110 });
        assert_eq!(p.add(Direction::West, 10), Position { x: 90, y: 100 });
        assert_eq!(p.add(Direction::East, 10), Position { x: 110, y: 100 });
    }

    #[test]
    fn test_player_coord_sub() {
        let p = Position { x: 100, y: 100 };

        assert_eq!(p.add(Direction::North, -10), Position { x: 100, y: 110 });
        assert_eq!(p.add(Direction::South, -10), Position { x: 100, y: 90 });
        assert_eq!(p.add(Direction::West, -10), Position { x: 110, y: 100 });
        assert_eq!(p.add(Direction::East, -10), Position { x: 90, y: 100 });
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
        let r = Ratios::new(2, 2, 2, 2, 2, 2, 2);

        assert_eq!(Cell::Upgrade(Upgrade::Power), r.generate(000));
        assert_eq!(Cell::Upgrade(Upgrade::Power), r.generate(001));
        assert_eq!(Cell::Upgrade(Upgrade::Speed), r.generate(002));
        assert_eq!(Cell::Upgrade(Upgrade::Speed), r.generate(003));
        assert_eq!(Cell::Upgrade(Upgrade::Bombs), r.generate(004));
        assert_eq!(Cell::Upgrade(Upgrade::Bombs), r.generate(005));
        assert_eq!(Cell::Upgrade(Upgrade::Schinken), r.generate(006));
        assert_eq!(Cell::Upgrade(Upgrade::Schinken), r.generate(007));
        assert_eq!(Cell::Teleport, r.generate(008));
        assert_eq!(Cell::Teleport, r.generate(009));
        assert_eq!(Cell::Wood, r.generate(010));
        assert_eq!(Cell::Wood, r.generate(011));
        assert_eq!(Cell::Clear, r.generate(012));
        assert_eq!(Cell::Clear, r.generate(013));
    }

    #[test]
    fn test_field_index_to_player_pos() {
        assert_eq!(Field::cell_index_to_position(5, 9), Position::new(550, 950));
    }

    #[test]
    fn test_player_pos_to_field_index() {
        let field = Field::new(11, 17);
        assert_eq!(
            field.position_to_cell_index(Position::new(599, 900)),
            Some((5, 9))
        );
        assert_eq!(
            field.position_to_cell_index(Position::new(0, 0)),
            Some((0, 0))
        );
        assert_eq!(
            field.position_to_cell_index(Position::new(1099, 1699)),
            Some((10, 16))
        );
        assert_eq!(
            field.position_to_cell_index(Position::new(1100, 1699)),
            None
        );
        assert_eq!(
            field.position_to_cell_index(Position::new(1099, 1700)),
            None
        );
        assert_eq!(field.position_to_cell_index(Position::new(0, -1)), None);
        assert_eq!(field.position_to_cell_index(Position::new(-1, 0)), None);
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
