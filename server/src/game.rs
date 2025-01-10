use crate::actor::{actor, Actor, Receiver};

pub enum Message {}

#[derive(Debug)]
struct Lobby {
    id: GameId,
    host: ClientId,
    settings: Settings,
    players: Vec<Player>,
}

#[derive(Debug)]
struct StartedGame {
    id: GameId,
    game_state: GameState,
    updates: Vec<Update>,
    future_updates: Vec<Update>,
    old_updates: Vec<Update>,
}

pub enum Game {
    Lobby(Lobby),
    Game(StartedGame),
}

impl Actor<Message> for Game {
    async fn handle_message(&mut self, message: Message) {
        todo!()
    }

    async fn close(self) {
        todo!()
    }
}
