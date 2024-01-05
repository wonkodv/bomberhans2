use crate::field::Cell;
use crate::field::Field;
use crate::field::Upgrade;
use crate::settings::Settings;
use crate::utils::random;
use crate::utils::CellPosition;
use crate::utils::Direction;
use crate::utils::Duration;
use crate::utils::Idx;
use crate::utils::PlayerId;
use crate::utils::Position;
use crate::utils::TimeStamp;
use std::fmt;
use std::rc::Rc;

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
    pub power: u32,

    /// current walking speed upgrades
    pub speed: u32,

    /// current bomb capacity upgrades
    pub bombs: u32,

    /// current placed bombs. Increased when placing, decreased when exploding.
    pub current_bombs_placed: u32,

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
        self.power = u32::max(1, self.power / 2);
        self.speed = u32::max(1, self.speed / 2);
        self.bombs = u32::max(1, self.bombs / 2);
        self.position = start_position;
        self.action = Action::idle();
    }

    fn score(&mut self, _killed: PlayerId) {
        self.kills += 1;
    }
}

/// Constants of an active Game
#[derive(Debug)]
pub struct Game {
    pub players: Vec<Player>,
    pub settings: Settings,
    pub local_player: PlayerId,
}

impl Game {
    pub fn new_local_game(settings: Settings) -> Self {
        let field = Field::new(settings.width, settings.height);
        let start_positions = field.start_positions();

        assert!(start_positions.len() >= settings.players as _);

        let local_player = PlayerId(0);

        let players: Vec<Player> = (0..(settings.players as usize))
            .map(|id| Player {
                name: {
                    if id == local_player.0 {
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
            players,
            settings,
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
            write!(f, " & placing")?;
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

        let field = Field::new_from_rules(&game.settings);

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
        let player_state = &self.player_states[player_id.0];

        let direction = player_state
            .action
            .walking
            .expect("only call walking if player is walking");

        let mut walk_distance = self
            .game
            .settings
            .get_update_walk_distance(player_state.speed)
            .try_into()
            .expect("walked distance fits i32");

        let current_cell_pos = player_state.position.as_cell_pos();
        let cell_ahead = &self.field[current_cell_pos.add(direction, 1)];
        let cell_ahead_left =
            &self.field[current_cell_pos.add(direction, 1).add(direction.left(), 1)];
        let cell_ahead_right =
            &self.field[current_cell_pos.add(direction, 1).add(direction.right(), 1)];

        if cell_ahead.walkable() {
            if !cell_ahead_left.walkable() {
                // TODO: move away from left wall by distance_to_border - ACC/5
            }
        } else {
            let distance_to_wall =
                player_state.position.distance_to_border(direction) - (Position::ACCURACY / 5);
            walk_distance = i32::min(distance_to_wall, walk_distance);
        }

        if walk_distance > 0 {
            let new_position = player_state.position.add(direction, walk_distance);
            self.walk_on_cell(player_id, new_position);
        }
    }

    fn walk_on_cell(&mut self, player_id: PlayerId, new_position: Position) {
        let player = &self.game.players[player_id.0];
        let player_state = &mut self.player_states[player_id.0];
        let cell_position = new_position.as_cell_pos();
        let cell = &self.field[cell_position];
        log::debug!(
            "{:?} {:?} @ {:?} walking to {:?} == {:?} ({:?}) ",
            self.time,
            player_id,
            player_state.position,
            new_position,
            cell_position,
            &cell
        );
        match *cell {
            Cell::StartPoint | Cell::Empty => {
                player_state.move_(new_position);
            }
            Cell::Bomb { .. } => {
                if random(self.time, new_position.x, new_position.y) % 100
                    < self.game.settings.bomb_walking_chance
                {
                    // GAME_RULE: walking on bombs randomly happens or doesn't, decided
                    // each update.
                    player_state.move_(new_position);
                }
            }
            Cell::TombStone { .. } => {
                if random(self.time, new_position.x, new_position.y) % 100
                    < self.game.settings.tombstone_walking_chance
                {
                    // GAME_RULE: walking on tombstones randomly happens or doesn't, decided
                    // each update.
                    player_state.move_(new_position);
                }
            }
            Cell::Fire { owner, .. } => {
                // GAME_RULE: walking into fire counts as kill by fire owner
                // TODO: seperate counter?
                player_state.die(owner, player.start_position);
                self.player_states[owner.0].score(player_id);
                self.field[cell_position] = Cell::TombStone(player_id);

                log::info!(
                    "{:?} {:?} @ {:?} suicided",
                    self.time,
                    player_id,
                    new_position,
                );
            }
            Cell::Upgrade(upgrade) => {
                player_state.move_(new_position);
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
                    let target = targets[random(self.time, new_position.x, new_position.y)
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
                Some(direction) => player_state.position.add(
                    direction,
                    -(self.game.settings.bomb_offset as i32 * 100 / Position::ACCURACY),
                ),
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
                        expire: self.time + self.game.settings.bomb_explode_time(),
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
            // TODO: Tombstone gives upgrade that player had most of?
            Cell::Fire { .. } | Cell::Empty | Cell::TombStone(..) => (true, 0, owner),
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

                (true, self.game.settings.upgrade_explosion_power, owner)
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
                (explodes, self.game.settings.upgrade_explosion_power, owner)
            }
            Cell::StartPoint | Cell::WoodBurning { .. } | Cell::Wall => (false, 0, owner),
            Cell::Wood => {
                let expire = self.time + self.game.settings.wood_burn_time();
                self.field[cell] = Cell::WoodBurning { expire };
                log::info!("{cell:?}: setting wall on fire until {expire:?}");
                (false, 0, owner)
            }
        };
        if explodes {
            self.field[cell] = Cell::Fire {
                owner,
                expire: self.time + self.game.settings.fire_burn_time(),
            };
            for (id, p) in self.player_states.iter_mut().enumerate() {
                if p.position.as_cell_pos() == cell {
                    p.die(owner, self.game.players[id].start_position);
                    self.field[cell] = Cell::TombStone(PlayerId(id));
                }
            }

            let power: isize = power.try_into().expect("power fits");
            if power > 0 {
                let x = cell.x as isize;
                let y = cell.y as isize;
                for (dx, dy) in [(-1, 0), (1, 0), (0, 1), (0, -1)] {
                    for i in 1..=power {
                        let x = x + dx * i;
                        let y = y + dy * i;
                        if x >= 0 && y >= 0 {
                            let pos = CellPosition::new(x as i32, y as i32);
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
                        *cell = self.game.settings.ratios.random(r);
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
        let settings = Settings::default();
        let game = Game {
            players: vec![player1],
            settings,
            local_player,
        };

        let game = Rc::new(game);

        let mut gs = State::new(game);
        gs.player_states[0].current_bombs_placed = 42; // Hack, so bombs can explode without int
                                                       // underrun. If a test cares, it should set
                                                       // this correctly
        gs
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
