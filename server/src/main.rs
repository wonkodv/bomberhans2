use std::io::Write;

use std::error::Error;
use std::net::UdpSocket;
use std::thread::sleep;

use bomberhans_lib::network::*;

mod server;

fn serve() -> Result<(), Box<dyn Error>> {
    let socket = UdpSocket::bind("0.0.0.0:4267")?;
    socket.set_nonblocking(true)?;

    let mut server = server::Server::new("HansServer".to_owned());

    let mut buf = [0; 1024];

    loop {
        for _ in 0..15 {
            match socket.recv_from(&mut buf) {
                Ok((received_bytes, client_address)) => {
                    if let Some(msg) = decode::<ClientMessage>(&buf[..received_bytes]) {
                        let response = server.handle_client_message(msg, client_address);
                        if let Some(response) = response {
                            let data = encode(&response);
                            socket.send_to(&data, client_address)?;
                        }
                    }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(e) => {
                    return Err(e.into());
                }
            }
            sleep(std::time::Duration::from_millis(1))
        }
        let updates = server.periodic_update();
        for (adr, msg) in updates {
            let data = encode(&msg);
            socket.send_to(&data, adr)?;
        }
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
    log::info!("Running Bomberhans Server {}", bomberhans_lib::VERSION);

    match serve() {
        Ok(()) => {}
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(1);
        }
    }
}
