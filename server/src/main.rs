use std::collections::HashMap;
use std::hash::Hash as _;
use std::hash::Hasher as _;
use std::io::Write;

use std::error::Error;
use std::net::SocketAddr;
use std::net::UdpSocket;
use std::rc::Rc;
use std::time::Instant;

use bomberhans_lib::game_state::*;
use bomberhans_lib::network::*;
use bomberhans_lib::utils::*;

struct Game {
    id: GameId,
    game_static: Rc<GameStatic>,
    server_state: GameState,
    updates: Vec<Update>,
}

struct Lobby {
    game_static: GameStatic,
}

enum ClientState {
    Lurking,
    Lobby(Rc<Lobby>),
    Game(Rc<Game>),
}

struct Client {
    pub cookie: ClientId,
    pub address: SocketAddr,
    pub state: ClientState,
    pub last_communication: Instant,
}

struct Server {
    name: String,
    games: HashMap<GameId, Rc<Game>>,
    lobbies: HashMap<LobbyId, Rc<Lobby>>,
    clients: HashMap<ClientId, Client>,
}

impl Server {
    fn new(name: String, socket: UdpSocket) -> Self {
        let games = HashMap::new();
        let lobbies = HashMap::new();
        let clients = HashMap::new();

        Self {
            name,
            games,
            lobbies,
            clients,
        }
    }

    fn handle_client_helo(
        &mut self,
        address: SocketAddr,
        update: ClientHello,
    ) -> Option<ServerHello> {
        let mut h = std::hash::DefaultHasher::new();
        address.hash(&mut h);
        update.name.hash(&mut h);
        let cookie = h.finish();
        let cookie = ClientId::new(cookie);

        let state = ClientState::Lurking;
        let last_communication = Instant::now();

        let client = Client {
            cookie,
            address,
            state,
            last_communication,
        };

        self.clients.insert(cookie, client);

        let name = self.name.clone();
        let games = self
            .games
            .value()
            .map(|g| (g.id, g.game_static.name.clone()));

        return Some(ServerHello {});
    }

    fn handle_client_update(&mut self, update: ClientUpdate) {}
}

fn serve() -> Result<(), Box<dyn Error>> {
    let s = UdpSocket::bind("0.0.0.0:4267")?;
    let mut buf = [0; 1024];

    loop {
        let (amt, src) = socket.recv_from(&mut buf)?;
        println!("Received {amt} bytes from {src}");

        socket.send_to(&buf[..amt], src)?;
    }
}

fn main() {
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
    log::info!(concat!(
        "Running Bomberhans Server ",
        env!("CARGO_PKG_VERSION")
    ));

    match serve() {
        Ok(()) => {}
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(1);
        }
    }
}
