use crate::actor::{actor, Actor, Receiver};

pub enum Command {

}

pub fn game() -> Actor {
    actor(|rx| Game {rx}.run())
}

struct Game {
    rx: Receiver<Command>,

}

impl Game {
    async fn run(self) {

    }
}
