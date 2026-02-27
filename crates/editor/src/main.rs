#![warn(clippy::all, rust_2018_idioms)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

/// Use mimalloc allocator for native builds
#[cfg(not(target_arch = "wasm32"))]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

// When compiling natively:
#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    enya_editor::run_native_app(None)
}

// When compiling to web using trunk:
#[cfg(target_arch = "wasm32")]
fn main() {
    use eframe::wasm_bindgen::JsCast as _;

    // Redirect `log` message to `console.log` and friends:
    eframe::WebLogger::init(log::LevelFilter::Debug).ok();

    // Block mobile browsers before loading the full app
    if is_mobile() {
        show_mobile_message();
        return;
    }

    let web_options = eframe::WebOptions::default();

    // On WASM, AsyncRuntime uses wasm-bindgen-futures (no external runtime needed)
    let async_runtime = enya_editor::AsyncRuntime::new();

    wasm_bindgen_futures::spawn_local(async move {
        let document = web_sys::window()
            .expect("No window")
            .document()
            .expect("No document");

        let canvas = document
            .get_element_by_id("the_canvas_id")
            .expect("Failed to find the_canvas_id")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("the_canvas_id was not a HtmlCanvasElement");

        let start_result = eframe::WebRunner::new()
            .start(
                canvas,
                web_options,
                Box::new(move |cc| {
                    egui_extras::install_image_loaders(&cc.egui_ctx);
                    Ok(Box::new(enya_editor::EnyaApp::new(cc, async_runtime)))
                }),
            )
            .await;

        if let Some(loading) = document.get_element_by_id("loading") {
            match start_result {
                Ok(_) => {
                    loading.remove();
                }
                Err(e) => {
                    loading.set_inner_html(
                        "<p> The app has crashed. See the developer console for details. </p>",
                    );
                    panic!("Failed to start eframe: {e:?}");
                }
            }
        }
    });
}

/// Detect mobile browsers via user-agent string or narrow viewport.
#[cfg(target_arch = "wasm32")]
fn is_mobile() -> bool {
    let Some(window) = web_sys::window() else {
        return false;
    };

    // Check user-agent for mobile indicators
    let has_mobile_ua = window
        .navigator()
        .user_agent()
        .ok()
        .map(|ua| {
            let ua = ua.to_lowercase();
            ua.contains("mobi") || ua.contains("android")
        })
        .unwrap_or(false);

    // Check viewport width (< 768px is typically a phone)
    let is_narrow = window
        .inner_width()
        .ok()
        .and_then(|w| w.as_f64())
        .map(|w| w < 768.0)
        .unwrap_or(false);

    has_mobile_ua || is_narrow
}

/// Replace the loading screen with a mobile-friendly message.
#[cfg(target_arch = "wasm32")]
fn show_mobile_message() {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Some(document) = window.document() else {
        return;
    };

    // Hide the canvas so only the message is visible
    if let Some(canvas) = document.get_element_by_id("the_canvas_id") {
        let _ = canvas.set_attribute("style", "display:none");
    }

    // Replace the loading indicator with a friendly message
    if let Some(loading) = document.get_element_by_id("loading") {
        loading.set_inner_html(
            r#"<p style="font-size:16px">
                <picture>
                    <source srcset="logo.png" media="(prefers-color-scheme: dark)">
                    <img src="logo.png" alt="Enya" width="50%">
                </picture>
            </p>
            <p class="mobile-message">Enya is designed for desktop</p>"#,
        );
    }
}
