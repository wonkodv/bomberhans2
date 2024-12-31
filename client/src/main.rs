use std::io::Write;
use std::time::Instant;

mod connection;
mod game;
mod gui;

fn main() {
    env_logger::Builder::from_default_env()
        .format(|buf, rec| {
            writeln!(
                buf,
                "{file}:{line}: {module} ({time:?}) {args}",
                file = rec.file().unwrap(),
                line = rec.line().unwrap(),
                module = rec.module_path().unwrap(),
                args = rec.args(),
                time = Instant::now(),
            )
        })
        .format_timestamp_micros()
        .init();
    log::info!("Running Bomberhans Client {}", bomberhans_lib::VERSION);
    gui::gui();
}
