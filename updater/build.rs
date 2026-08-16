use std::{env, path::PathBuf};

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("manifest directory"));
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        let source = manifest.join("macos/authorization.m");
        println!("cargo:rerun-if-changed={}", source.display());
        println!("cargo:rustc-link-lib=framework=Foundation");
        println!("cargo:rustc-link-lib=framework=AppKit");
        println!("cargo:rustc-link-lib=framework=Security");
        println!("cargo:rustc-link-lib=framework=ServiceManagement");
        cc::Build::new()
            .file(source)
            .flag("-fobjc-arc")
            .compile("superiority_updater_authorization");
    }
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        build_windows_resources(&manifest);
    }
}

fn build_windows_resources(manifest: &std::path::Path) {
    let app_resources = manifest.join("../app/windows");
    let manifest_file = app_resources.join("Superiority.manifest.xml");
    let icon_file = app_resources.join("Superiority.ico");
    let output = PathBuf::from(env::var("OUT_DIR").expect("build output directory"));
    let resource_file = output.join("SuperiorityUpdater.rc");
    let version = env::var("SUPERIORITY_APP_VERSION")
        .unwrap_or_else(|_| env::var("CARGO_PKG_VERSION").expect("package version"));
    println!("cargo:rerun-if-env-changed=SUPERIORITY_APP_VERSION");
    println!("cargo:rerun-if-changed={}", manifest_file.display());
    println!("cargo:rerun-if-changed={}", icon_file.display());
    let mut parts = version
        .split('.')
        .map(|part| part.parse::<u16>().unwrap_or(0))
        .collect::<Vec<_>>();
    parts.resize(4, 0);
    let resources = format!(
        r#"#define RT_MANIFEST 24
1 RT_MANIFEST "{}"
1 ICON "{}"
1 VERSIONINFO
 FILEVERSION {},{},{},{}
 PRODUCTVERSION {},{},{},{}
 FILEFLAGSMASK 0x3fL
 FILEOS 0x40004L
 FILETYPE 0x1L
BEGIN
    BLOCK "StringFileInfo"
    BEGIN
        BLOCK "040904b0"
        BEGIN
            VALUE "CompanyName", "Superiority\0"
            VALUE "FileDescription", "Superiority Updater\0"
            VALUE "FileVersion", "{}\0"
            VALUE "ProductName", "Superiority\0"
            VALUE "ProductVersion", "{}\0"
        END
    END
    BLOCK "VarFileInfo"
    BEGIN
        VALUE "Translation", 0x0409, 1200
    END
END
"#,
        rc_path(&manifest_file),
        rc_path(&icon_file),
        parts[0],
        parts[1],
        parts[2],
        parts[3],
        parts[0],
        parts[1],
        parts[2],
        parts[3],
        version,
        version,
    );
    std::fs::write(&resource_file, resources).expect("write Windows resources");
    embed_resource::compile(&resource_file, embed_resource::NONE)
        .manifest_required()
        .expect("compile Windows updater resources");
}

fn rc_path(path: &std::path::Path) -> String {
    path.to_string_lossy()
        .replace('\\', "/")
        .replace('"', "\\\"")
}
