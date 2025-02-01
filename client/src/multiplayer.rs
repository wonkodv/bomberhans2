use std::net::SocketAddr;

use bomberhans2_lib::network::GameId;

enum State {
    Initial,
    Connecting,
    ServerView,
    JoiningLobby,
    InLobby,
    InGame,
    GameOver,
}

enum UserEvents {
    /// User clicked Connect
    Connect(SocketAddr),

    /// User clicked Join
    Join(GameId),

    /// User clicked Disconnect
    Disconnect,
}

enum ServerEvents {
    LobbyUpdate,
}
