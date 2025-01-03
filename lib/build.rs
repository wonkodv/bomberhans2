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

    let cargo_version = concat!("v", env!("CARGO_PKG_VERSION"));
    if !git_version.starts_with(cargo_version) {
        println!("cargo:warning=expected git-version {git_version:?} to match cargo-version {cargo_version:?}");
    }
    println!("cargo:rustc-env=VERSION={git_version}");

    // TODO: The following is super annoying if nothing ahs changed !
    // let output = Command::new("touch") // modify self to re-run git describe next time
    //     .arg("build.rs")
    //     .spawn()
    //     .expect("can start touch");
}
