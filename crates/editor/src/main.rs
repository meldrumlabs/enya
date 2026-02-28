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

    let web_options = eframe::WebOptions::default();

    // On WASM, AsyncRuntime uses wasm-bindgen-futures (no external runtime needed)
    let async_runtime = enya_editor::AsyncRuntime::new();

    wasm_bindgen_futures::spawn_local(async move {
        let Some(window) = web_sys::window() else {
            log::error!("No window object available");
            return;
        };
        let Some(document) = window.document() else {
            log::error!("No document object available");
            return;
        };

        let Some(canvas) = document.get_element_by_id("the_canvas_id") else {
            log::error!("Failed to find the_canvas_id element");
            return;
        };
        let Ok(canvas) = canvas.dyn_into::<web_sys::HtmlCanvasElement>() else {
            log::error!("the_canvas_id was not a HtmlCanvasElement");
            return;
        };

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
