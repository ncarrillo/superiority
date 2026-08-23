//! a standalone battle.net sign-in window.
//!
//! this is a separate process on purpose. the webview needs an event loop on
//! the main thread — appkit will not have it any other way — and a library
//! loaded into someone else's runtime does not own main. running here means an
//! embedder gets a sign-in window without surrendering its own main thread, and
//! without a webview sharing an address space with a host garbage collector.
//!
//! reads the authentication url from stdin — argv is readable by any local
//! user through `ps` — then prints the session token to stdout and exits 0, or
//! exits non-zero.

use std::{cell::RefCell, io::Read as _, process::ExitCode, rc::Rc};

use tao::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder},
    platform::run_return::EventLoopExtRunReturn as _,
    window::WindowBuilder,
};
use url::Url;
use wry::WebViewBuilder;
#[cfg(target_os = "macos")]
use wry::WebViewBuilderExtDarwin as _;

/// its own cookie jar, distinct from the app's and from every other webview on
/// the machine. a supporting crate must not sign someone in off a session it
/// did not establish, nor leave one behind for something else to pick up.
#[cfg(target_os = "macos")]
const DATA_STORE: [u8; 16] = [
    0x1B, 0x9E, 0x54, 0xC2, 0x7A, 0x33, 0x4D, 0x61, 0xB8, 0x2F, 0x0C, 0xE7, 0x95, 0x14, 0xAD, 0x38,
];

/// battle.net ends the flow by navigating to a port-0 loopback url that nothing
/// can actually serve, so the token is read off the navigation attempt rather
/// than from a listener.
fn callback_token(location: &str) -> Option<String> {
    let url = Url::parse(location).ok()?;
    if url.scheme() != "http" || url.host_str() != Some("localhost") || url.port() != Some(0) {
        return None;
    }
    let token = url
        .query_pairs()
        .find_map(|(name, value)| (name == "ST").then(|| value.into_owned()))?;
    let bytes = token.as_bytes();
    if bytes.is_empty() || bytes.len() > 1024 || !bytes.iter().all(u8::is_ascii_graphic) {
        return None;
    }
    Some(token)
}

fn main() -> ExitCode {
    let mut target = String::new();
    if std::io::stdin().read_to_string(&mut target).is_err() {
        eprintln!("could not read the authentication url from stdin");
        return ExitCode::FAILURE;
    }
    let target = target.trim().to_owned();
    if target.is_empty() || Url::parse(&target).is_err() {
        eprintln!("stdin did not carry a url");
        return ExitCode::FAILURE;
    }

    match present(&target) {
        Ok(Some(token)) => {
            println!("{token}");
            ExitCode::SUCCESS
        }
        Ok(None) => {
            eprintln!("sign-in window closed before completing");
            ExitCode::FAILURE
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn present(target: &str) -> Result<Option<String>, String> {
    let mut event_loop = EventLoopBuilder::new().build();
    let window = WindowBuilder::new()
        .with_title("Battle.net sign-in")
        .with_inner_size(tao::dpi::LogicalSize::new(980.0, 760.0))
        .build(&event_loop)
        .map_err(|error| format!("could not open a window: {error}"))?;

    let token: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
    let captured = Rc::clone(&token);

    let proxy = event_loop.create_proxy();
    let builder = WebViewBuilder::new().with_url(target);
    #[cfg(target_os = "macos")]
    let builder = builder.with_data_store_identifier(DATA_STORE);
    let _webview = builder
        .with_navigation_handler(move |location| match callback_token(&location) {
            // returning false blocks the navigation, so the window never tries
            // to load a url nothing is listening on.
            Some(found) => {
                *captured.borrow_mut() = Some(found);
                let _ = proxy.send_event(());
                false
            }
            None => true,
        })
        .build(&window)
        .map_err(|error| format!("could not open a webview: {error}"))?;

    let mut outcome = None;
    event_loop.run_return(|event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            Event::UserEvent(()) => {
                outcome = token.borrow_mut().take();
                *control_flow = ControlFlow::Exit;
            }
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => *control_flow = ControlFlow::Exit,
            _ => {}
        }
    });
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::callback_token;

    #[test]
    fn extracts_the_port_zero_battle_net_callback() {
        assert_eq!(
            callback_token("http://localhost:0/?ST=US-test-token").as_deref(),
            Some("US-test-token")
        );
    }

    #[test]
    fn rejects_anything_that_is_not_the_callback() {
        assert!(callback_token("http://localhost:8080/?ST=US-test-token").is_none());
        assert!(callback_token("https://localhost:0/?ST=US-test-token").is_none());
        assert!(callback_token("http://example.com:0/?ST=US-test-token").is_none());
        assert!(callback_token("http://localhost:0/?ST=").is_none());
    }
}
