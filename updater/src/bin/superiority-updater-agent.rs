#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use std::{
    fs,
    io::{Read as _, Write as _},
    path::{Path, PathBuf},
    process::{Child, Command, ExitCode, Stdio},
    thread,
    time::{Duration, Instant},
};

use superiority_updater::{InstallPlan, Result};

const TRUSTED_APPLICATION_ID: &str = "com.superiority.sc2-chat";
const TRUSTED_PUBLIC_KEY: &str = "IVqqIejocXACpzUqr/W4FpT8qkuJidILS7UqPZ7x7xE=";
const RESULT_TIMEOUT: Duration = Duration::from_secs(30 * 60);

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            println!("ERROR:{error}");
            let _ = std::io::stdout().flush();
            log_failure(&error.to_string());
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<()> {
    let mut arguments = std::env::args_os();
    let _program = arguments.next();
    match arguments.next().as_deref() {
        Some(mode) if mode == std::ffi::OsStr::new("--install-plan") => {
            let plan_path = required_path(&mut arguments, "install plan")?;
            ensure_no_arguments(arguments)?;
            run_user_agent(&plan_path)
        }
        Some(mode) if mode == std::ffi::OsStr::new("--execute-plan") => {
            let plan_path = required_path(&mut arguments, "install plan")?;
            if arguments.next().as_deref() != Some(std::ffi::OsStr::new("--result")) {
                return Err(superiority_updater::Error::Installation(
                    "the privileged updater worker requires --result".into(),
                ));
            }
            let result_path = required_path(&mut arguments, "install result")?;
            ensure_no_arguments(arguments)?;
            run_privileged_worker(&plan_path, &result_path)
        }
        Some(mode) if mode == std::ffi::OsStr::new("--show-progress-after") => {
            let delay = arguments
                .next()
                .and_then(|value| value.to_string_lossy().parse::<u64>().ok())
                .ok_or_else(|| {
                    superiority_updater::Error::Installation("the progress delay is invalid".into())
                })?;
            if arguments.next().as_deref() != Some(std::ffi::OsStr::new("--app-name")) {
                return Err(superiority_updater::Error::Installation(
                    "the progress helper requires --app-name".into(),
                ));
            }
            let app_name = arguments.next().ok_or_else(|| {
                superiority_updater::Error::Installation("the application name is missing".into())
            })?;
            if arguments.next().as_deref() != Some(std::ffi::OsStr::new("--app-path")) {
                return Err(superiority_updater::Error::Installation(
                    "the progress helper requires --app-path".into(),
                ));
            }
            let app_path = required_path(&mut arguments, "application")?;
            ensure_no_arguments(arguments)?;
            thread::sleep(Duration::from_millis(delay));
            superiority_updater::show_progress(&app_name.to_string_lossy(), &app_path)
        }
        _ => Err(superiority_updater::Error::Installation(
            "the updater agent requires --install-plan or --execute-plan".into(),
        )),
    }
}

fn run_user_agent(plan_path: &Path) -> Result<()> {
    let plan = InstallPlan::read(plan_path)?;
    validate_product_identity(&plan)?;
    plan.validate_prepared_update()?;
    let result_path = plan_path.with_file_name("install-result.json");
    let elevated = superiority_updater::installation_needs_authorization(&plan.install_target);
    if elevated {
        superiority_updater::launch_elevated_worker(
            plan_path,
            &result_path,
            &plan.app_name,
            &plan.install_target,
        )?;
    }
    println!("READY");
    std::io::stdout()
        .flush()
        .map_err(|error| superiority_updater::Error::io("updater stdout", error))?;

    superiority_updater::wait_for_process(plan.parent_pid)?;
    let progress = ProgressProcess::start(&plan);
    if elevated {
        wait_for_install_result(&result_path)?;
    } else {
        plan.execute()?;
    }
    drop(progress);
    superiority_updater::relaunch(&plan.relaunch_path)?;
    clean_session(&plan, plan_path);
    Ok(())
}

struct ProgressProcess {
    child: Option<Child>,
}

impl ProgressProcess {
    fn start(plan: &InstallPlan) -> Self {
        let child = std::env::current_exe().ok().and_then(|agent| {
            Command::new(agent)
                .arg("--show-progress-after")
                .arg(plan.fallback_progress_delay_ms.to_string())
                .arg("--app-name")
                .arg(&plan.app_name)
                .arg("--app-path")
                .arg(&plan.install_target)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .ok()
        });
        Self { child }
    }
}

impl Drop for ProgressProcess {
    fn drop(&mut self) {
        if let Some(child) = &mut self.child {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn run_privileged_worker(plan_path: &Path, result_path: &Path) -> Result<()> {
    if result_path.parent() != plan_path.parent()
        || result_path.file_name() != Some(std::ffi::OsStr::new("install-result.json"))
    {
        return Err(superiority_updater::Error::Installation(
            "the install result path is outside the updater session".into(),
        ));
    }
    let outcome = (|| {
        let plan = InstallPlan::read(plan_path)?;
        validate_product_identity(&plan)?;
        let privileged = plan.prepare_for_privileged_install()?;
        let outcome = (|| {
            superiority_updater::wait_for_process(privileged.parent_pid)?;
            privileged.execute()
        })();
        privileged.cleanup_privileged_staging();
        outcome
    })();
    write_install_result(result_path, outcome.as_ref().err().map(ToString::to_string))?;
    outcome
}

fn validate_product_identity(plan: &InstallPlan) -> Result<()> {
    if plan.application_id != TRUSTED_APPLICATION_ID || plan.public_key != TRUSTED_PUBLIC_KEY {
        return Err(superiority_updater::Error::Installation(
            "the install plan does not target a trusted Superiority release".into(),
        ));
    }
    Ok(())
}

fn wait_for_install_result(path: &Path) -> Result<()> {
    let started = Instant::now();
    loop {
        if path.is_file() {
            let mut result = String::new();
            fs::File::open(path)
                .and_then(|mut file| file.read_to_string(&mut result))
                .map_err(|error| superiority_updater::Error::io(path, error))?;
            let value: serde_json::Value = serde_json::from_str(&result)?;
            if value.get("ok").and_then(serde_json::Value::as_bool) == Some(true) {
                return Ok(());
            }
            return Err(superiority_updater::Error::Installation(
                value
                    .get("error")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("the privileged installer failed")
                    .to_owned(),
            ));
        }
        if started.elapsed() >= RESULT_TIMEOUT {
            return Err(superiority_updater::Error::Installation(
                "the privileged installer did not finish within 30 minutes".into(),
            ));
        }
        thread::sleep(Duration::from_millis(50));
    }
}

fn write_install_result(path: &Path, error: Option<String>) -> Result<()> {
    let temporary = path.with_extension("json.part");
    let value = match error {
        Some(error) => serde_json::json!({ "ok": false, "error": error }),
        None => serde_json::json!({ "ok": true }),
    };
    let mut options = fs::OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o644).custom_flags(libc::O_NOFOLLOW);
    }
    let mut file = options
        .open(&temporary)
        .map_err(|error| superiority_updater::Error::io(&temporary, error))?;
    file.write_all(value.to_string().as_bytes())
        .map_err(|error| superiority_updater::Error::io(&temporary, error))?;
    file.sync_all()
        .map_err(|error| superiority_updater::Error::io(&temporary, error))?;
    fs::rename(&temporary, path).map_err(|error| superiority_updater::Error::io(path, error))?;
    Ok(())
}

fn clean_session(plan: &InstallPlan, plan_path: &Path) {
    if plan.staging_directory.exists() {
        let _ = fs::remove_dir_all(&plan.staging_directory);
    }
    if let Some(session) = plan_path.parent() {
        // the running executable lives here on purpose. removing a running
        // file is supported on macos; windows cleans a locked stale session on
        // the next launch.
        let _ = fs::remove_dir_all(session);
    }
}

fn required_path(
    arguments: &mut impl Iterator<Item = std::ffi::OsString>,
    label: &str,
) -> Result<PathBuf> {
    arguments.next().map(PathBuf::from).ok_or_else(|| {
        superiority_updater::Error::Installation(format!("the {label} path is missing"))
    })
}

fn ensure_no_arguments(mut arguments: impl Iterator<Item = std::ffi::OsString>) -> Result<()> {
    if arguments.next().is_some() {
        return Err(superiority_updater::Error::Installation(
            "the updater agent received unexpected arguments".into(),
        ));
    }
    Ok(())
}

fn log_failure(message: &str) {
    let Some(cache) = dirs::cache_dir() else {
        return;
    };
    let directory = cache.join("com.superiority.sc2-chat").join("Updater");
    if fs::create_dir_all(&directory).is_ok() {
        let _ = fs::write(directory.join("last-error.log"), message);
    }
}
