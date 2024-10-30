use std::io::Write;

use std::error::Error;
use std::net::UdpSocket;

fn serve() -> Result<(), Box<dyn Error>> {
    let socket = UdpSocket::bind("0.0.0.0:4267")?;
    let mut buf = [0; 1024];

    loop {
        let (amt, src) = socket.recv_from(&mut buf)?;
        println!("Received {amt} bytes from {src}");

        socket.send_to(&buf[..amt], src)?;
    }
}

fn main() {
    env_logger::Builder::from_default_env()
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
