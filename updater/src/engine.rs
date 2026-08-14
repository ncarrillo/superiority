use std::{
    fmt::Write as _,
    fs,
    io::{BufRead as _, BufReader},
    path::{Path, PathBuf},
    process::{Command as ProcessCommand, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    thread,
};

use rand::Rng as _;
use semver::Version;
use url::Url;

use crate::{
    Appcast, Error, Event, InstallPlan, Platform, PreparedUpdate, Release, Result, download,
    platform,
};

#[derive(Clone, Debug)]
pub struct Config {
    pub feed_url: Url,
    pub public_key: String,
    pub current_version: String,
    pub current_build: String,
    pub app_name: String,
    pub application_id: String,
    pub platform: Platform,
    pub system_version: Option<String>,
    pub install_target: PathBuf,
    pub relaunch_path: PathBuf,
    pub cache_directory: PathBuf,
    pub agent_path: PathBuf,
}

impl Config {
    pub fn for_current_process(
        feed_url: &str,
        public_key: &str,
        current_version: &str,
        app_name: &str,
        application_id: &str,
    ) -> Result<Self> {
        let platform = Platform::current()
            .ok_or_else(|| Error::UnsupportedPlatform(std::env::consts::OS.into()))?;
        let (install_target, relaunch_path, cache_directory) =
            platform::default_paths(application_id)?;
        let current_build = Version::parse(current_version).map_or_else(
            |_| current_version.to_owned(),
            |version| version.patch.to_string(),
        );
        let agent_path = default_agent_path(&install_target, platform);
        Ok(Self {
            feed_url: Url::parse(feed_url)
                .map_err(|error| Error::InvalidAppcast(format!("invalid feed URL: {error}")))?,
            public_key: public_key.to_owned(),
            current_version: current_version.to_owned(),
            current_build,
            app_name: app_name.to_owned(),
            application_id: application_id.to_owned(),
            platform,
            system_version: platform::system_version(),
            install_target,
            relaunch_path,
            cache_directory,
            agent_path,
        })
    }
}

pub struct Client {
    commands: Sender<Command>,
    cancelled: Arc<AtomicBool>,
}

impl Client {
    #[must_use]
    pub fn start(config: Config) -> (Self, Receiver<Event>) {
        let (commands, command_receiver) = mpsc::channel();
        let (events, event_receiver) = mpsc::channel();
        let cancelled = Arc::new(AtomicBool::new(false));
        let worker_cancelled = Arc::clone(&cancelled);
        thread::Builder::new()
            .name("superiority-updater".into())
            .spawn(move || run(&config, &command_receiver, &events, &worker_cancelled))
            .expect("the updater worker thread must start");
        (
            Self {
                commands,
                cancelled,
            },
            event_receiver,
        )
    }

    pub fn check(&self) {
        self.cancelled.store(false, Ordering::Release);
        let _ = self.commands.send(Command::Check);
    }

    pub fn primary_action(&self) {
        self.cancelled.store(false, Ordering::Release);
        let _ = self.commands.send(Command::PrimaryAction);
    }

    pub fn dismiss(&self) {
        self.cancelled.store(true, Ordering::Release);
        let _ = self.commands.send(Command::Dismiss);
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
        let _ = self.commands.send(Command::Shutdown);
    }
}

enum Command {
    Check,
    PrimaryAction,
    Dismiss,
    Shutdown,
}

enum State {
    Idle,
    Available(Release),
    Ready(PreparedUpdate),
    Installing,
}

fn run(
    config: &Config,
    commands: &Receiver<Command>,
    events: &Sender<Event>,
    cancelled: &Arc<AtomicBool>,
) {
    let mut state = State::Idle;
    while let Ok(command) = commands.recv() {
        match command {
            Command::Shutdown => break,
            Command::Dismiss => {
                if let State::Ready(prepared) = &state {
                    clean_prepared(prepared, &config.cache_directory);
                }
                state = State::Idle;
                send(events, Event::Dismissed);
            }
            Command::Check => {
                if matches!(
                    state,
                    State::Available(_) | State::Ready(_) | State::Installing
                ) {
                    send(events, Event::Focus);
                    continue;
                }
                send(events, Event::Checking);
                match check_for_update(config, cancelled) {
                    Ok(Some(release)) => {
                        send_available(events, &release);
                        state = State::Available(release);
                    }
                    Ok(None) => {
                        send(
                            events,
                            Event::NotFound {
                                message: format!(
                                    "Superiority {} is the newest available version.",
                                    config.current_version
                                ),
                            },
                        );
                    }
                    Err(Error::Cancelled) => send(events, Event::Dismissed),
                    Err(error) => send_error(events, &error),
                }
            }
            Command::PrimaryAction => match state {
                State::Available(ref release) => {
                    match prepare_update(config, release.clone(), cancelled, events) {
                        Ok(prepared) => {
                            send(events, Event::Ready);
                            state = State::Ready(prepared);
                        }
                        Err(Error::Cancelled) => {
                            send(events, Event::Dismissed);
                            state = State::Idle;
                        }
                        Err(error) => {
                            send_error(events, &error);
                            state = State::Idle;
                        }
                    }
                }
                State::Ready(ref prepared) => match launch_agent(config, prepared) {
                    Ok(()) => {
                        send(events, Event::Installing);
                        send(events, Event::QuitRequested);
                        state = State::Installing;
                    }
                    Err(error) => send_error(events, &error),
                },
                State::Idle => {
                    send_error(events, &Error::InvalidState("no update has been selected"));
                }
                State::Installing => send(events, Event::Focus),
            },
        }
    }
}

fn check_for_update(config: &Config, cancelled: &AtomicBool) -> Result<Option<Release>> {
    let xml = download::fetch_appcast(&config.feed_url, cancelled)?;
    let appcast = Appcast::parse(&xml, config.platform)?;
    Ok(appcast
        .latest_newer_than(&config.current_build, config.system_version.as_deref())
        .cloned())
}

fn prepare_update(
    config: &Config,
    release: Release,
    cancelled: &Arc<AtomicBool>,
    events: &Sender<Event>,
) -> Result<PreparedUpdate> {
    let downloads = config.cache_directory.join("Downloads");
    let staging_root = config.cache_directory.join("Staging");
    fs::create_dir_all(&downloads).map_err(|error| Error::io(&downloads, error))?;
    fs::create_dir_all(&staging_root).map_err(|error| Error::io(&staging_root, error))?;
    let archive_path = downloads.join(format!(
        "{}-{}.zip",
        safe_name(&release.version),
        release.build
    ));
    let cached_is_valid = archive_path.is_file()
        && download::verify_artifact(&release.artifact, &config.public_key, &archive_path).is_ok();
    if cached_is_valid {
        send(events, Event::Downloading { progress: 1.0 });
    } else {
        if archive_path.exists() {
            fs::remove_file(&archive_path).map_err(|error| Error::io(&archive_path, error))?;
        }
        download::download_artifact(&release.artifact, &archive_path, cancelled, |progress| {
            send(events, Event::Downloading { progress });
        })?;
        download::verify_artifact(&release.artifact, &config.public_key, &archive_path)?;
    }

    let staging_directory = staging_root.join(format!("{}-{}", release.build, random_token()));
    let extraction = platform::extract(&archive_path, &staging_directory, cancelled, |progress| {
        send(events, Event::Extracting { progress });
    });
    let install_source = match extraction {
        Ok(source) => source,
        Err(error) => {
            remove_directory_if_owned(&staging_directory, &config.cache_directory);
            return Err(error);
        }
    };
    platform::validate_install_source(&install_source, &config.application_id)?;
    Ok(PreparedUpdate {
        release,
        archive_path,
        staging_directory,
        install_source,
    })
}

fn launch_agent(config: &Config, prepared: &PreparedUpdate) -> Result<()> {
    let sessions = config.cache_directory.join("Sessions");
    fs::create_dir_all(&sessions).map_err(|error| Error::io(&sessions, error))?;
    let session = sessions.join(random_token());
    fs::create_dir(&session).map_err(|error| Error::io(&session, error))?;
    let agent_name = if cfg!(target_os = "windows") {
        "superiority-updater-agent.exe"
    } else {
        "superiority-updater-agent"
    };
    let copied_agent = session.join(agent_name);
    fs::copy(&config.agent_path, &copied_agent).map_err(|error| {
        Error::Installation(format!(
            "copy updater agent from {}: {error}",
            config.agent_path.display()
        ))
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(&copied_agent, fs::Permissions::from_mode(0o700))
            .map_err(|error| Error::io(&copied_agent, error))?;
    }
    let plan = InstallPlan::new(
        &config.app_name,
        &config.application_id,
        config.platform,
        prepared,
        &config.install_target,
        &config.relaunch_path,
        &config.public_key,
    );
    let plan_path = plan.write_securely(&session)?;
    let mut child = ProcessCommand::new(&copied_agent)
        .arg("--install-plan")
        .arg(plan_path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| Error::io(&copied_agent, error))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| Error::Installation("updater agent did not open its status pipe".into()))?;
    let mut status = String::new();
    BufReader::new(stdout)
        .read_line(&mut status)
        .map_err(|error| Error::io("updater agent status", error))?;
    if status.trim() == "READY" {
        return Ok(());
    }
    let _ = child.wait();
    Err(Error::Installation(
        status
            .trim()
            .strip_prefix("ERROR:")
            .unwrap_or("the updater agent did not become ready")
            .to_owned(),
    ))
}

fn send_available(events: &Sender<Event>, release: &Release) {
    send(
        events,
        Event::Available {
            version: release.version.clone(),
            title: release.title.clone(),
            notes: release.notes.clone(),
            notes_format: release.notes_format.clone(),
            size: release.artifact.content_length,
        },
    );
}

fn send_error(events: &Sender<Event>, error: &Error) {
    send(
        events,
        Event::Error {
            message: error.to_string(),
        },
    );
}

fn random_token() -> String {
    let random: [u8; 12] = rand::rng().random();
    random
        .iter()
        .fold(String::with_capacity(24), |mut token, byte| {
            let _ = write!(token, "{byte:02x}");
            token
        })
}

fn send(events: &Sender<Event>, event: Event) {
    let _ = events.send(event);
}

fn clean_prepared(prepared: &PreparedUpdate, cache: &Path) {
    remove_directory_if_owned(&prepared.staging_directory, cache);
}

fn remove_directory_if_owned(directory: &Path, cache: &Path) {
    if directory.starts_with(cache) && directory != cache {
        let _ = fs::remove_dir_all(directory);
    }
}

fn safe_name(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect()
}

fn default_agent_path(target: &Path, platform: Platform) -> PathBuf {
    match platform {
        Platform::MacOs => target.join("Contents/Helpers/superiority-updater-agent"),
        Platform::WindowsX86_64 | Platform::WindowsAarch64 => {
            target.join("superiority-updater-agent.exe")
        }
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use std::{
        fs,
        io::{Read as _, Write as _},
        net::TcpListener,
        os::unix::fs::PermissionsExt as _,
        process::Command,
        sync::mpsc::RecvTimeoutError,
        thread,
        time::{Duration, Instant},
    };

    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use ed25519_dalek::{Signer as _, SigningKey};
    use sha2::{Digest as _, Sha256};

    use super::{Client, Config};
    use crate::{Event, Platform};

    #[test]
    fn checks_downloads_verifies_and_extracts_a_macos_update() {
        let temporary = tempfile::tempdir().unwrap();
        let app = temporary.path().join("Superiority.app");
        let contents = app.join("Contents");
        let executable = contents.join("MacOS/superiority");
        fs::create_dir_all(executable.parent().unwrap()).unwrap();
        fs::write(
            contents.join("Info.plist"),
            br#"<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>com.superiority.sc2-chat</string>
<key>CFBundleExecutable</key><string>superiority</string>
<key>CFBundlePackageType</key><string>APPL</string>
</dict></plist>"#,
        )
        .unwrap();
        fs::write(&executable, b"#!/bin/sh\nexit 0\n").unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(
            Command::new("/usr/bin/codesign")
                .args(["--force", "--sign", "-"])
                .arg(&app)
                .status()
                .unwrap()
                .success()
        );

        let archive = temporary.path().join("update.zip");
        assert!(
            Command::new("/usr/bin/ditto")
                .args(["-c", "-k", "--keepParent"])
                .arg(&app)
                .arg(&archive)
                .status()
                .unwrap()
                .success()
        );
        let archive_bytes = fs::read(&archive).unwrap();
        let signing_key = SigningKey::from_bytes(&[7_u8; 32]);
        let signature = STANDARD.encode(signing_key.sign(&archive_bytes).to_bytes());
        let hash = format!("{:x}", Sha256::digest(&archive_bytes));

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let artifact_url = format!("http://{address}/update.zip");
        let feed = format!(
            r#"<?xml version="1.0"?><rss xmlns:sparkle="http://www.andymatuschak.org/xml-namespaces/sparkle" version="2.0"><channel><item>
<title>0.1.27</title><sparkle:version>27</sparkle:version><sparkle:shortVersionString>0.1.27</sparkle:shortVersionString>
<description sparkle:format="markdown"><![CDATA[# Ready]]></description>
<enclosure url="{artifact_url}" length="{}" sparkle:edSignature="{signature}" sha256="{hash}" />
</item></channel></rss>"#,
            archive_bytes.len()
        );
        let responses = [feed.into_bytes(), archive_bytes];
        let server = thread::spawn(move || {
            for body in responses {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = [0_u8; 4096];
                let _ = stream.read(&mut request).unwrap();
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                )
                .unwrap();
                stream.write_all(&body).unwrap();
            }
        });

        let config = Config {
            feed_url: format!("http://{address}/appcast.xml").parse().unwrap(),
            public_key: STANDARD.encode(signing_key.verifying_key().to_bytes()),
            current_version: "0.1.26".into(),
            current_build: "26".into(),
            app_name: "Superiority".into(),
            application_id: "com.superiority.sc2-chat".into(),
            platform: Platform::MacOs,
            system_version: Some("14.0".into()),
            install_target: temporary.path().join("Installed.app"),
            relaunch_path: temporary.path().join("Installed.app"),
            cache_directory: temporary.path().join("Cache"),
            agent_path: temporary.path().join("unused-agent"),
        };
        let (client, events) = Client::start(config);
        client.check();
        assert!(matches!(
            events.recv_timeout(Duration::from_secs(2)).unwrap(),
            Event::Checking
        ));
        assert!(matches!(
            events.recv_timeout(Duration::from_secs(2)).unwrap(),
            Event::Available { version, .. } if version == "0.1.27"
        ));
        client.primary_action();
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            match events.recv_timeout(remaining) {
                Ok(Event::Ready) => break,
                Ok(Event::Error { message }) => panic!("update preparation failed: {message}"),
                Ok(_) => {}
                Err(RecvTimeoutError::Timeout) => panic!("update preparation timed out"),
                Err(RecvTimeoutError::Disconnected) => panic!("updater worker disconnected"),
            }
        }
        server.join().unwrap();
    }
}
