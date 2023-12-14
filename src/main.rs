mod game;
mod gui;
mod network;
mod rules;
mod utils;

use std::io::Write;
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
    log::info!(concat!("Running Bomberhans ", env!("VERSION")));
    gui::gui();
}
