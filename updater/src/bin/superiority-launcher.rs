#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

#[cfg(target_os = "windows")]
fn main() -> std::process::ExitCode {
    match launch() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            let _ = std::fs::write("superiority-launch-error.log", error.to_string());
            std::process::ExitCode::FAILURE
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn main() {
    eprintln!("The Superiority launcher is only used on Windows.");
}

#[cfg(target_os = "windows")]
fn launch() -> superiority_updater::Result<()> {
    use std::path::{Component, Path, PathBuf};

    #[derive(serde::Deserialize)]
    struct CurrentVersion {
        schema: u32,
        executable: PathBuf,
        previous_executable: Option<PathBuf>,
    }

    fn validate(path: &Path) -> superiority_updater::Result<()> {
        if path.is_absolute()
            || path
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
            || path.extension().is_none_or(|extension| extension != "exe")
        {
            return Err(superiority_updater::Error::Installation(
                "the active executable path is unsafe".into(),
            ));
        }
        Ok(())
    }

    let launcher = std::env::current_exe()
        .map_err(|error| superiority_updater::Error::io("launcher", error))?;
    let root = launcher.parent().ok_or_else(|| {
        superiority_updater::Error::Installation("the launcher has no parent directory".into())
    })?;
    let manifest_path = root.join("current.json");
    let manifest: CurrentVersion = serde_json::from_slice(
        &std::fs::read(&manifest_path)
            .map_err(|error| superiority_updater::Error::io(&manifest_path, error))?,
    )?;
    if manifest.schema != 1 {
        return Err(superiority_updater::Error::Installation(
            "the active-version manifest schema is unsupported".into(),
        ));
    }
    validate(&manifest.executable)?;
    let arguments = std::env::args_os().skip(1).collect::<Vec<_>>();
    if std::process::Command::new(root.join(&manifest.executable))
        .args(&arguments)
        .spawn()
        .is_ok()
    {
        return Ok(());
    }
    let previous = manifest.previous_executable.ok_or_else(|| {
        superiority_updater::Error::Installation(
            "the active application failed to launch and no rollback is available".into(),
        )
    })?;
    validate(&previous)?;
    std::process::Command::new(root.join(previous))
        .args(arguments)
        .spawn()
        .map_err(|error| {
            superiority_updater::Error::Installation(format!("rollback launch: {error}"))
        })?;
    Ok(())
}
