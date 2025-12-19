#![warn(clippy::all, rust_2018_idioms)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

/// Use mimalloc allocator for native builds
#[cfg(not(target_arch = "wasm32"))]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

// When compiling natively:
#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    env_logger::init(); // Log to stderr (if you run with `RUST_LOG=debug`).

    // Setup a CryptoProvider to be able to use wss connections
    match rustls::crypto::ring::default_provider().install_default() {
        Ok(()) => {} // Do nothing crypto provider install successful
        Err(_) => panic!("failed to install CryptoProvider"),
    }

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([800.0, 600.0])
            .with_icon(
                enya_editor::util::png_to_icon_data(&include_bytes!("../assets/logo.png")[..]), //.expect("Failed to load icon"),
            )
            // Custom titlebar with custom traffic light buttons (close/minimize/fullscreen)
            // drawn in app.rs for seamless theme integration
            .with_titlebar_shown(false)
            .with_titlebar_buttons_shown(false)
            .with_fullsize_content_view(true)
            // Set app identifier for Wayland and macOS app identification
            .with_app_id("Enya"),
        ..Default::default()
    };

    eframe::run_native(
        "",
        native_options,
        Box::new(|cc| Ok(Box::new(enya_editor::EnyaApp::new(cc)))),
    )
}

// When compiling to web using trunk:
#[cfg(target_arch = "wasm32")]
fn main() {
    use eframe::wasm_bindgen::JsCast as _;

    // Redirect `log` message to `console.log` and friends:
    eframe::WebLogger::init(log::LevelFilter::Debug).ok();

    let web_options = eframe::WebOptions::default();

    wasm_bindgen_futures::spawn_local(async {
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
                Box::new(|cc| {
                    egui_extras::install_image_loaders(&cc.egui_ctx);
                    Ok(Box::new(enya_editor::EnyaApp::new(cc)))
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
