mod bncs;
mod error;
mod event;
mod sc2;
mod server;
mod tui;
mod web_auth;

use std::{
    io::{self, IsTerminal as _},
    net::SocketAddr,
    process::ExitCode,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};

use clap::Parser;

use crate::{
    error::{Error, Result},
    event::{Event, Reporter},
};

#[derive(Debug, Parser)]
#[command(
    name = "gateway",
    version,
    about = "BNCS compatibility gateway backed by Superiority's SC2 client"
)]
struct Arguments {
    #[arg(
        long,
        default_value = "127.0.0.1:6112",
        value_name = "IP:PORT",
        help = "Address on which legacy bots connect"
    )]
    listen: SocketAddr,

    #[arg(
        long,
        help = "Permit a non-loopback listener; required for a bridged VM"
    )]
    allow_remote: bool,

    #[arg(
        long,
        default_value_t = 45,
        value_name = "SECONDS",
        help = "Maximum time for SC2 network startup after authentication"
    )]
    timeout: u64,

    #[arg(
        long,
        default_value_t = 300,
        value_name = "SECONDS",
        help = "Maximum time to leave the Battle.net login window open"
    )]
    login_timeout: u64,

    #[arg(
        long,
        help = "Ignore Superiority's cached Battle.net credential and show login"
    )]
    force_login: bool,

    #[arg(long, help = "Disable the terminal interface")]
    no_tui: bool,
}

fn main() -> ExitCode {
    match run(&Arguments::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("gateway: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(arguments: &Arguments) -> Result<()> {
    validate(arguments)?;
    let interactive = !arguments.no_tui && io::stdin().is_terminal() && io::stdout().is_terminal();
    let stop = Arc::new(AtomicBool::new(false));

    let (sender, receiver) = mpsc::channel();
    let reporter = Reporter::new(sender);
    let (authentication, authentication_requests) = web_auth::channel();
    let signal_stop = Arc::clone(&stop);
    ctrlc::set_handler(move || signal_stop.store(true, Ordering::Release))?;

    let configuration = server::Configuration {
        listen: arguments.listen,
        force_login: arguments.force_login,
        startup_timeout: Duration::from_secs(arguments.timeout),
        login_timeout: Duration::from_secs(arguments.login_timeout),
        authentication,
    };
    let worker_stop = Arc::clone(&stop);
    let worker_reporter = reporter.clone();
    let worker = thread::Builder::new()
        .name("gateway-server".into())
        .spawn(move || {
            let result = server::run(&configuration, &worker_stop, &worker_reporter);
            let _ = worker_reporter.send(Event::Finished(
                result.as_ref().err().map(ToString::to_string),
            ));
            result
        })?;
    drop(reporter);

    let interface_result = if interactive {
        tui::run(&receiver, &authentication_requests, &stop)
    } else {
        tui::run_headless(&receiver, &authentication_requests);
        Ok(())
    };
    stop.store(true, Ordering::Release);
    let worker_result = worker.join().map_err(|_| Error::WorkerPanicked)?;
    interface_result?;
    worker_result
}

fn validate(arguments: &Arguments) -> Result<()> {
    if !arguments.listen.ip().is_loopback() && !arguments.allow_remote {
        return Err(Error::RemoteListenerNotAllowed(arguments.listen));
    }
    if arguments.timeout == 0 {
        return Err(Error::Configuration(
            "--timeout must be greater than zero".into(),
        ));
    }
    if arguments.login_timeout == 0 {
        return Err(Error::Configuration(
            "--login-timeout must be greater than zero".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remote_listener_requires_explicit_permission() {
        let arguments = Arguments::try_parse_from(["gateway", "--listen", "0.0.0.0:6112"])
            .expect("valid arguments");
        assert!(matches!(
            validate(&arguments),
            Err(Error::RemoteListenerNotAllowed(_))
        ));
    }

    #[test]
    fn zero_timeouts_are_rejected() {
        let arguments =
            Arguments::try_parse_from(["gateway", "--timeout", "0"]).expect("valid arguments");
        assert!(matches!(validate(&arguments), Err(Error::Configuration(_))));
    }
}
