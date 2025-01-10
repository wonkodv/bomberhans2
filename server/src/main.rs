use std::io::Write;

use std::net::Ipv6Addr;
use std::net::SocketAddr;

mod actor;
mod server;

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

    let server_name = "HansServer".to_string();
    let server = server::server(server_name);
}
