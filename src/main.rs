#![cfg_attr(
    debug_assertions,
    allow(dead_code, unused_variables, unreachable_code)
)]

mod game;
mod gui;
mod network;
mod utils;

fn main() {
    env_logger::init();
    gui::gui();
}
