use std::future::Future;
use std::io::Write;
use std::net::Ipv6Addr;
use std::net::SocketAddr;
use tokio::sync::mpsc::{channel, Sender};
use tokio::task::JoinHandle;

mod server;
mod actor;
mod game;

pub fn actor<F, M>(message_handler: F) -> (Sender<M>, JoinHandle<()>)
where
    M: Send,
    F: FnMut(M) -> Future<Output = ()> + Send,
{
    let (tx, mut rx) = channel(8);

    let jh = tokio::spawn(async move {
        loop {
            match rx.recv().await {
                None => return,
                Some(command) => (message_handler)(command).await,
            }
        }
    });

    return (tx, jh);
}

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format(|buf, rec| {
            writeln!(
                buf,
                "{file}:{line}: {module} {args}",
                file = rec.file().unwrap(),
                line = rec.line().unwrap(),
                module = rec.module_path().unwrap(),
                args = rec.args()
            )
        })
        .init();
    log::info!("Running Bomberhans Server {}", bomberhans_lib::VERSION);

    let addr = SocketAddr::new(Ipv6Addr::UNSPECIFIED.into(), 4267); // TODO: make port / ip configurable
    let socket = tokio::net::UdpSocket::bind(addr);
    log::info!("Listening on {addr}");

    let server = server::Server::new("HansServer".to_string());
}


