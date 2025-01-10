use tokio::{
    sync::mpsc::{channel, Sender},
    task::JoinHandle,
};

enum Message<M> {
    Message(M),
    Close,
}

#[derive(Debug)]
pub struct Manager<M: Send> {
    tx: Sender<Message<M>>,
    join: JoinHandle<()>,
}

#[derive(Debug, Clone)]
pub struct AssistantManager<M: Send> {
    tx: Sender<Message<M>>,
}

impl<M: Send> AssistantManager<M> {
    pub async fn send_message(&self, message: M) {
        self.tx.send(Message::Message(message)).await.unwrap();
    }
}

impl<M: Send> Manager<M> {
    pub async fn send_message(&self, message: M) {
        self.tx.send(Message::Message(message)).await.unwrap();
    }

    pub async fn close(self) {
        self.tx.send(Message::Close).await.unwrap();
        self.join.await;
    }

    pub fn assistant(&self) -> AssistantManager<M> {
        AssistantManager {
            tx: self.tx.clone(),
        }
    }
}

pub fn spawn<M, A: Actor<M>>(actor: A) -> Manager<M> {
    let (tx, mut rx) = channel(8);

    let join = tokio::spawn(async move {
        loop {
            match rx.recv().await {
                None => return,
                Some(Message::Message(message)) => {
                    actor.handle_message(message).await;
                }
                Some(Message::Close) => {
                    break;
                }
            }
        }
        actor.close().await;
    });

    Manager { tx, join }
}

pub trait Actor<M>
where
    M: Send,
{
    async fn handle_message(&mut self, message: M);
    async fn close(self);
}
