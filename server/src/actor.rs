use std::future::Future;
use tokio::sync::mpsc::{channel, Sender};

pub enum Done {
    NotDone,
    Done,
}

#[derive(Debug, Clone)]
pub struct Manager<C: Send> {
    tx: Sender<C>,
}

impl<C: Send> Manager<C> {
    pub async fn send_message(&self, message: C) {
        self.tx.send(message).await.unwrap();
    }
}

pub fn actor<M,A: Actor<M>>(actor: A) -> Manager<M> {
    let (tx, mut rx) = channel(8);

    tokio::spawn(async move {
        loop {
            match rx.recv().await {
                None => return,
                Some(command) => match (handle_message)(command).await {
                    Done::NotDone => (),
                    Done::Done => return,
                },
            }
        }
    });

    Manager { tx }
}

trait Actor<M>
where
    M: Send,
{
    async fn handle_message(message: M) -> Done;
}
