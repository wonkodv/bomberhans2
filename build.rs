use std::process::Command;
fn main() {
    let output = Command::new("git")
        .arg("describe")
        .arg("--tags")
        .output()
        .expect("git describe works");
    let git_version = String::from_utf8(output.stdout).expect("git dewcribe gives utf8");

    let cargo_version = concat!("v", env!("CARGO_PKG_VERSION"));
    if !git_version.starts_with(cargo_version) {
        panic!(
            "expected git-version {git_version:?} to start with cargo-version {cargo_version:?}"
        );
    }
    println!("cargo:rustc-env=VERSION={}", git_version);
}
