fn main() {
    println!("cargo:rerun-if-env-changed=STIMPAK_PACKAGE_VERSION");
    let version = std::env::var("STIMPAK_PACKAGE_VERSION")
        .ok()
        .filter(|version| !version.is_empty())
        .unwrap_or_else(|| std::env::var("CARGO_PKG_VERSION").unwrap());
    println!("cargo:rustc-env=STIMPAK_EFFECTIVE_VERSION={version}");
}
