use bomberhans_lib::field::Field;
use bomberhans_lib::game_state::{Action, GameState, Player};
use bomberhans_lib::settings::Settings;
use bomberhans_lib::utils::{GameTime, PlayerId, Position, TIME_PER_TICK};
use std::collections::VecDeque;
use std::time;

#[derive(Debug)]
pub struct MultiPlayerGame {
    server_state: GameState,
    local_actions: VecDeque<(GameTime, Action)>,
    local_state: GameState,
    last_local_update: std::time::Instant,
}

impl MultiPlayerGame {}

#[derive(Debug, Clone)]
pub struct SinglePlayerGame {
    game_state: GameState,
    last_update: std::time::Instant,
    local_player: PlayerId,
}

impl SinglePlayerGame {
    pub fn new(settings: Settings) -> Self {
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

        let game_state = GameState::new(settings, players);

        SinglePlayerGame {
            game_state,
            local_player,
            last_update: time::Instant::now(),
        }
    }

    /// proceed game time according to real time since last update
    pub fn update_simulation_realtime(&mut self) {
        let now = time::Instant::now();
        while now >= self.last_update + TIME_PER_TICK {
            self.last_update += TIME_PER_TICK;
            self.game_state.simulate_1_update();
        }
    }

    pub fn set_local_player_action(&mut self, action: Action) {
        self.game_state.set_player_action(self.local_player, action);
    }

    pub fn game_state(&self) -> &GameState {
        &self.game_state
    }
}
