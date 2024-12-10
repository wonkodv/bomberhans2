use std::io::Write;

mod game;
mod gui;

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
    log::info!("Running Bomberhans Client {}", bomberhans_lib::VERSION);
    gui::gui();
}
