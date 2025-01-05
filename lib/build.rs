use std::process::Command;

fn main() {
    let output = Command::new("git")
        .arg("describe")
        .arg("--tags")
        .arg("--dirty")
        .arg("--match")
        .arg("v*")
        .output()
        .expect("git describe works");
    let git_version = String::from_utf8(output.stdout).expect("git describe gives utf8");
    let git_version = git_version.trim();

    let cargo_version = format!("v{}", std::env::var("CARGO_PKG_VERSION").unwrap());
    if !git_version.starts_with(&cargo_version) {
        println!("cargo:warning=expected git-version {git_version:?} to start with cargo-version {cargo_version:?}");
    }

    println!("cargo:rustc-env=VERSION={git_version}");
}
