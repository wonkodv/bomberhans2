use tokio::{
    sync::mpsc::{channel, Sender},
    task::JoinHandle,
};

enum Instruction<M> {
    Instruction(M),
    Close,
}

#[derive(Debug)]
pub struct Manager<M: Send> {
    tx: Sender<Instruction<M>>,
    join: JoinHandle<()>,
}

impl<M: Send> Manager<M> {
    pub async fn send(&self, message: M) {
        self.tx
            .send(Instruction::Instruction(message))
            .await
            .unwrap();
    }

    pub async fn close(self) {
        self.tx.send(Instruction::Close).await.unwrap();
        self.join.await;
    }

    pub fn assistant(&self) -> AssistantManager<M> {
        AssistantManager {
            tx: self.tx.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct AssistantManager<M: Send> {
    tx: Sender<Instruction<M>>,
}

impl<M: Send> AssistantManager<M> {
    pub async fn send(&self, instruction: M) {
        self.tx
            .send(Instruction::Instruction(instruction))
            .await
            .unwrap();
    }
}

/// Start an actor.
///
/// Spawn a tokio task that receives instructions from a channel.
/// return a manager, that can send instructions to that queue.
/// manager can also close the actor, or hand out assistants. assistants can only send
/// instructions, not close the actor.
pub fn launch<I, A>(actor: A) -> Manager<I>
where
    I: Send + 'static,
    A: Actor<I> + Send,
{
    let (tx, mut rx) = channel(8);
    let mut actor = actor;
    let join = tokio::spawn(async move {
        loop {
            match rx.recv().await {
                None => return,
                Some(Instruction::Instruction(instruction)) => {
                    actor.handle(instruction).await;
                }
                Some(Instruction::Close) => {
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
    async fn handle(&mut self, message: M);
    async fn close(self);
}
