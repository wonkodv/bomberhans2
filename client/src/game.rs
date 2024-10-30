use bomberhans_lib::field::Field;
use bomberhans_lib::game_state::{Action, GameState, GameStatic, Player};
use bomberhans_lib::settings::Settings;
use bomberhans_lib::utils::{PlayerId, Position, TimeStamp, TIME_PER_TICK};
use std::collections::VecDeque;
use std::rc::Rc;
use std::time;

#[derive(Debug)]
pub struct MultiPlayerGame {
    game_static: Rc<GameStatic>,
    server_state: GameState,
    local_actions: VecDeque<(TimeStamp, Action)>,
    local_state: GameState,
    last_local_update: std::time::Instant,
}

impl MultiPlayerGame {
    /// proceed game time according to real time since last update
    fn update_local_simulation_realtime(&mut self) {
        let now = time::Instant::now();
        while now >= self.last_local_update + TIME_PER_TICK {
            self.last_local_update += TIME_PER_TICK;
            self.local_state.simulate_1_update();
        }
    }

    pub fn set_local_player_action(&mut self, action: Action) {
        self.local_state
            .set_player_action(self.game_static.local_player, action);
        self.local_actions
            .push_back((self.local_state.time, action));
        // TODO: send to server
    }
}

#[derive(Debug)]
pub struct SinglePlayerGame {
    game_static: Rc<GameStatic>,
    game_state: GameState,
    last_update: std::time::Instant,
}

impl SinglePlayerGame {
    /// proceed game time according to real time since last update
    fn update_simulation_realtime(&mut self) {
        let now = time::Instant::now();
        while now >= self.last_update + TIME_PER_TICK {
            self.last_update += TIME_PER_TICK;
            self.game_state.simulate_1_update();
        }
    }

    pub fn set_local_player_action(&mut self, action: Action) {
        self.game_state
            .set_player_action(self.game_static.local_player, action);
    }
}

#[derive(Debug)]
pub enum Game {
    SinglePlayer(SinglePlayerGame),
    MultiPlayer(MultiPlayerGame),
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

        let game_static = GameStatic {
            players,
            settings,
            local_player,
        };
        let game_static = Rc::new(game_static);
        let game_state = GameState::new(Rc::clone(&game_static));

        Game::SinglePlayer(SinglePlayerGame {
            game_state,
            game_static,
            last_update: time::Instant::now(),
        })
    }

    pub fn new_multiplayer_game(
        settings: Settings,
        socket: (),
        local_player: PlayerId,
        players: Vec<Player>,
    ) -> Self {
        let game_static = GameStatic {
            players,
            settings,
            local_player,
        };
        let state = GameState::new(Rc::new(game_static));

        todo!()
    }

    pub fn set_local_player_action(&mut self, action: Action) {
        match self {
            Game::SinglePlayer(spg) => spg.set_local_player_action(action),
            Game::MultiPlayer(mpg) => todo!(),
        }
    }

    pub fn settings(&self) -> &Settings {
        match self {
            Game::SinglePlayer(spg) => &spg.game_static.settings,
            Game::MultiPlayer(mpg) => &mpg.game_static.settings,
        }
    }

    pub fn stat(&self) -> &GameStatic {
        match self {
            Game::SinglePlayer(spg) => &spg.game_static,
            Game::MultiPlayer(mpg) => &mpg.game_static,
        }
    }

    pub fn local_state(&mut self) -> &GameState {
        match self {
            Game::SinglePlayer(spg) => {
                spg.update_simulation_realtime(); // TODO: is this hacky? where to put
                                                  // this? I don't want an extra thread
                                                  // for this
                &spg.game_state
            }
            Game::MultiPlayer(mpg) => &mpg.local_state,
        }
    }
}
