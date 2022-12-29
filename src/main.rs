#![cfg_attr(debug_assertions, allow(dead_code, unused_imports, unused_variables))]

mod game;
mod gui;
mod network;

fn main() {
    env_logger::init();
    gui::gui();
}
