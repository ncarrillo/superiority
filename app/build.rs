use std::{env, path::PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::consts::OS == "windows"
    {
        build_windows_manifest()?;
    }
    Ok(())
}

fn build_windows_manifest() -> Result<(), Box<dyn std::error::Error>> {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    let resources = manifest.join("windows");
    let manifest_file = resources.join("Superiority.manifest.xml");
    let icon_file = resources.join("Superiority.ico");
    let resource_file = PathBuf::from(env::var("OUT_DIR")?).join("Superiority.rc");
    let version = env::var("CARGO_PKG_VERSION")?;

    println!("cargo:rerun-if-changed={}", manifest_file.display());
    println!("cargo:rerun-if-changed={}", icon_file.display());
    std::fs::write(
        &resource_file,
        windows_resources(&manifest_file, &icon_file, &version, "Superiority"),
    )?;
    embed_resource::compile(&resource_file, embed_resource::NONE)
        .manifest_required()
        .map_err(|error| format!("compile Windows application manifest: {error}"))?;
    Ok(())
}

fn windows_resources(
    manifest: &std::path::Path,
    icon: &std::path::Path,
    version: &str,
    description: &str,
) -> String {
    let mut parts = version
        .split('.')
        .map(|part| part.parse::<u16>().unwrap_or(0))
        .collect::<Vec<_>>();
    parts.resize(4, 0);
    format!(
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
            VALUE "FileDescription", "{}\0"
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
        rc_path(manifest),
        rc_path(icon),
        parts[0],
        parts[1],
        parts[2],
        parts[3],
        parts[0],
        parts[1],
        parts[2],
        parts[3],
        description,
        version,
        version,
    )
}

fn rc_path(path: &std::path::Path) -> String {
    path.to_string_lossy()
        .replace('\\', "/")
        .replace('"', "\\\"")
}
