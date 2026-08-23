#![cfg(target_arch = "wasm32")]

mod products;
mod shell;

use std::borrow::Cow;

use gpui::{App, AppCell, AppContext as _, Application, WindowOptions};
use wasm_bindgen::prelude::*;

use shell::LiveShell;

#[wasm_bindgen]
pub fn run(feed_id: String, backend: String, asset_root: String) -> Result<(), JsValue> {
    console_error_panic_hook::set_once();
    let _ = console_log::init_with_level(log::Level::Info);

    gpui_platform::web_init();
    let app = keep_web_application_alive(gpui_platform::single_threaded_web());
    app.run(move |cx: &mut App| {
        let fonts = vec![
            Cow::Borrowed(
                include_bytes!("../../app/macos/resources/fonts/eurostileext-med.otf").as_slice(),
            ),
            Cow::Borrowed(
                include_bytes!("../../app/macos/resources/fonts/eurostile-reg.otf").as_slice(),
            ),
            // the SC:R card title asks for Eurostile Bold; without the bold face
            // the browser falls back to a plain sans, so embed it too
            Cow::Borrowed(
                include_bytes!("../../app/macos/resources/fonts/eurostile-bol.otf").as_slice(),
            ),
            Cow::Borrowed(include_bytes!("../../app/macos/resources/fonts/bl.ttf").as_slice()),
            // Reforged's UI face (Friz Quadrata), extracted from the WC3:R client,
            // so the WC3 card and realm read in the game's own type, not a fallback
            Cow::Borrowed(include_bytes!("../../app/macos/resources/fonts/frizqt.ttf").as_slice()),
        ];
        cx.text_system()
            .add_fonts(fonts)
            .expect("superiority fonts must load");

        cx.open_window(WindowOptions::default(), |_, cx| {
            cx.new(|cx| LiveShell::new(backend, feed_id, asset_root, cx))
        })
        .expect("the browser window must open");
        cx.activate(true);
    });

    Ok(())
}

fn keep_web_application_alive(app: Application) -> Application {
    // the web runtime requires one strong app cell for the page lifetime
    struct WasmApplication(std::rc::Rc<AppCell>);
    let wasm_app = unsafe { std::mem::transmute::<Application, WasmApplication>(app) };
    std::mem::forget(wasm_app.0.clone());
    unsafe { std::mem::transmute::<WasmApplication, Application>(wasm_app) }
}
