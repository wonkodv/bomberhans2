#![cfg_attr(
    debug_assertions,
    allow(
        dead_code,
        unused_variables,
        unused_mut,
        unused_assignments,
        unused_imports,
        unused_macros,
        unused_extern_crates,
        missing_docs,
    )
)]
pub mod field;
pub mod game_state;
pub mod network;
pub mod settings;
pub mod utils;

pub static VERSION: &str = env!("VERSION");

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn version_matches_cargo_version() {
        let cargo_version = concat!("v", env!("CARGO_PKG_VERSION"));
        assert!(
            VERSION.starts_with(cargo_version),
            "Expected git-version {VERSION} to start with {cargo_version}. Did you forget to `git tag`? "
        );
    }
}
