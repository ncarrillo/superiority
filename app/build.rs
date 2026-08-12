use std::{env, path::PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if env::var_os("CARGO_FEATURE_SPARKLE").is_some() {
        let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
        build_sparkle_bridge(&manifest)?;
    }
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        build_windows_manifest()?;
    }
    Ok(())
}

fn build_windows_manifest() -> Result<(), Box<dyn std::error::Error>> {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    let resources = manifest.join("windows");
    let manifest_file = resources.join("Superiority.manifest.xml");
    let resource_file = resources.join("Superiority.rc");

    println!("cargo:rerun-if-changed={}", manifest_file.display());
    println!("cargo:rerun-if-changed={}", resource_file.display());
    embed_resource::compile(&resource_file, embed_resource::NONE)
        .manifest_required()
        .map_err(|error| format!("compile Windows application manifest: {error}"))?;
    Ok(())
}

fn build_sparkle_bridge(manifest: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    let framework = PathBuf::from(env::var("SUPERIORITY_SPARKLE_FRAMEWORK")?);
    let framework_parent = framework
        .parent()
        .ok_or("Sparkle framework path has no parent directory")?;
    let headers = framework.join("Headers");
    let bridge = manifest.join("macos/sparkle_bridge.m");

    println!("cargo:rerun-if-changed={}", bridge.display());
    println!("cargo:rerun-if-env-changed=SUPERIORITY_SPARKLE_FRAMEWORK");
    println!(
        "cargo:rustc-link-search=framework={}",
        framework_parent.display()
    );
    println!("cargo:rustc-link-lib=framework=Sparkle");
    println!("cargo:rustc-link-arg=-Wl,-rpath,@executable_path/../Frameworks");

    cc::Build::new()
        .file(bridge)
        .include(headers)
        .flag("-fobjc-arc")
        .flag("-fblocks")
        .flag(format!("-F{}", framework_parent.display()))
        .compile("superiority_sparkle_bridge");
    Ok(())
}
