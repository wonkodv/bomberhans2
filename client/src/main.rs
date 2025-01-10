use std::io::Write;
use std::time::Instant;

use app::controller;

mod app;
mod communication;
mod game;
mod gui;
mod multiplayer;

// TODO: Tokio Main
fn main() {
    let start = Instant::now();

    env_logger::Builder::from_default_env()
        .format_timestamp_micros()
        .format(move |buf, rec| {
            writeln!(
                buf,
                "{file}:{line}: {module} ({time:#}) {args}",
                file = rec.file().unwrap(),
                line = rec.line().unwrap(),
                module = rec.module_path().unwrap(),
                args = rec.args(),
                time = start.elapsed().as_millis(),
            )
        })
        .init();
    log::info!("Running Bomberhans Client {}", bomberhans_lib::VERSION);

    let (game_controller, mut game_controller_backend) = controller();

    let runtime = tokio::runtime::Runtime::new().unwrap();

    let ctrl = runtime.spawn(async move {
        game_controller_backend.run().await;
    });


    // TODO: spawn_blocking
    gui::gui(game_controller);
}


// TODO: coordinated shutdsown
