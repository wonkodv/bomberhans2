use std::future::Future;

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

    pub async fn close(&self) {
        // try to send a close. if we can't, its because actor already closed
        let _ = self.tx.send(Instruction::Close).await;
    }

    pub async fn join(self) {
        self.join.await.expect("can wait on actor task");
    }

    pub async fn close_and_join(self) {
        self.close().await;
        self.join().await;
    }

    pub fn assistant(&self) -> AssistantManager<M> {
        AssistantManager {
            tx: self.tx.clone(),
        }
    }
}

#[derive(Debug)]
pub struct AssistantManager<M: Send> {
    tx: Sender<Instruction<M>>,
}

impl<M: Send> AssistantManager<M> {
    pub async fn send(&self, instruction: M) {
        self.tx
            .send(Instruction::Instruction(instruction))
            .await
            .expect("actor hasn't paniced {self:?}");
    }
    pub fn assistant(&self) -> AssistantManager<M> {
        AssistantManager {
            tx: self.tx.clone(),
        }
    }
}

/// Start an actor.
///
/// Spawn a tokio task that receives instructions from a channel.
/// return a manager, that can send instructions to that queue.
/// manager can also close the actor, or hand out assistants. assistants can only send
/// instructions, not close the actor.
pub fn launch<I, F, A>(actor: F) -> Manager<I>
where
    I: Send + 'static,
    A: Actor<I> + Send + 'static,
    F: FnOnce(AssistantManager<I>) -> A,
{
    let (tx, mut rx) = channel(8);
    let mut actor = actor(AssistantManager { tx: tx.clone() });
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
    fn handle(&mut self, message: M) -> impl Future<Output = ()> + Send;
    fn close(self) -> impl std::future::Future<Output = ()> + Send;
}
