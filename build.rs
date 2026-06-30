use std::process::Command;

fn main() {
    // Bake the version from git so the binary always reports the real tag/commit
    // it was built from instead of a hand-maintained constant. `git describe`
    // yields the latest reachable tag, plus `-<n>-g<hash>` when ahead of it and
    // `-dirty` with uncommitted changes — so a release build off a clean tag reads
    // e.g. `v0.19.0`, while a working build reads `v0.18.0-5-gabc123-dirty`.
    // Falls back to the Cargo.toml version when git isn't available (tarball build).
    let cargo_ver = std::env::var("CARGO_PKG_VERSION").unwrap_or_default();
    let version = Command::new("git")
        .args(["describe", "--tags", "--always", "--dirty"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| format!("v{cargo_ver}"));

    println!("cargo:rustc-env=SPLITUX_VERSION={version}");

    // Re-run when HEAD moves or tags change so the baked version stays current.
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs/tags");
    println!("cargo:rerun-if-changed=.git/packed-refs");
}
