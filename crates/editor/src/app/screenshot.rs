//! Screenshot handling.
//!
//! This module handles capturing and saving screenshots of the editor.
//! On native platforms, screenshots are saved to disk. On WASM, they
//! are triggered as browser downloads.

use crate::components::{Notification, NotificationLevel};

use super::EnyaApp;

impl EnyaApp {
    /// Handle screenshot events from egui
    pub(super) fn handle_screenshot_events(&mut self, ctx: &egui::Context) {
        ctx.input(|i| {
            for event in &i.raw.events {
                if let egui::Event::Screenshot { image, .. } = event {
                    self.save_screenshot(image);
                }
            }
        });
    }

    /// Save a screenshot image to disk
    pub(super) fn save_screenshot(&mut self, image: &std::sync::Arc<egui::ColorImage>) {
        use crate::util::now_unix_secs;

        // Generate filename with timestamp (works on both native and WASM)
        let timestamp = now_unix_secs();
        let filename = format!("enya_screenshot_{timestamp}.png");

        // Get the save path (custom path or default to Pictures directory)
        #[cfg(not(target_arch = "wasm32"))]
        let save_path = {
            if let Some(custom_path) = self.pending_screenshot_path.take() {
                let path = std::path::PathBuf::from(&custom_path);
                // If it's a directory, append the filename
                if path.is_dir() {
                    path.join(&filename)
                } else {
                    // Use as-is (user specified full path with filename)
                    path
                }
            } else {
                // Default: save to Pictures or home directory
                let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
                let pictures_dir = std::path::PathBuf::from(&home).join("Pictures");
                if pictures_dir.exists() {
                    pictures_dir.join(&filename)
                } else {
                    std::path::PathBuf::from(&home).join(&filename)
                }
            }
        };

        // Convert ColorImage to image buffer
        let width = image.width() as u32;
        let height = image.height() as u32;
        let pixels: Vec<u8> = image
            .pixels
            .iter()
            .flat_map(|c| [c.r(), c.g(), c.b(), c.a()])
            .collect();

        // Save the image
        match image::RgbaImage::from_raw(width, height, pixels) {
            Some(img_buffer) => {
                #[cfg(not(target_arch = "wasm32"))]
                match img_buffer.save(&save_path) {
                    Ok(()) => {
                        log::info!("Screenshot saved to: {}", save_path.display());
                        self.notifications.notify(Notification::new(
                            format!("Screenshot saved: {}", save_path.display()),
                            NotificationLevel::Success,
                        ));
                    }
                    Err(e) => {
                        log::error!("Failed to save screenshot: {e}");
                        self.notifications.notify(Notification::new(
                            format!("Failed to save screenshot: {e}"),
                            NotificationLevel::Error,
                        ));
                    }
                }

                #[cfg(target_arch = "wasm32")]
                {
                    // For WASM, trigger a browser download
                    match Self::trigger_browser_download(&filename, &img_buffer) {
                        Ok(()) => {
                            log::info!("Screenshot download triggered: {filename}");
                            self.notifications.notify(Notification::new(
                                format!("Screenshot downloading: {filename}"),
                                NotificationLevel::Success,
                            ));
                        }
                        Err(e) => {
                            log::error!("Failed to trigger download: {e}");
                            self.notifications.notify(Notification::new(
                                format!("Failed to download screenshot: {e}"),
                                NotificationLevel::Error,
                            ));
                        }
                    }
                }
            }
            None => {
                log::error!("Failed to create image buffer from screenshot");
                self.notifications.notify(Notification::new(
                    "Failed to create screenshot image".to_string(),
                    NotificationLevel::Error,
                ));
            }
        }
    }

    /// Trigger a browser download for the screenshot (WASM only)
    #[cfg(target_arch = "wasm32")]
    fn trigger_browser_download(
        filename: &str,
        img_buffer: &image::RgbaImage,
    ) -> Result<(), String> {
        use std::io::Cursor;
        use wasm_bindgen::JsCast;

        // Encode the image as PNG
        let mut png_data = Vec::new();
        img_buffer
            .write_to(&mut Cursor::new(&mut png_data), image::ImageFormat::Png)
            .map_err(|e| format!("Failed to encode PNG: {e}"))?;

        // Create a Blob from the PNG data
        let uint8_array = js_sys::Uint8Array::from(png_data.as_slice());
        let array = js_sys::Array::new();
        array.push(&uint8_array);

        let options = web_sys::BlobPropertyBag::new();
        options.set_type("image/png");

        let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&array, &options)
            .map_err(|e| format!("Failed to create Blob: {e:?}"))?;

        // Create an object URL for the blob
        let url = web_sys::Url::create_object_url_with_blob(&blob)
            .map_err(|e| format!("Failed to create object URL: {e:?}"))?;

        // Create a temporary anchor element and trigger download
        let window = web_sys::window().ok_or("No window")?;
        let document = window.document().ok_or("No document")?;
        let anchor: web_sys::HtmlAnchorElement = document
            .create_element("a")
            .map_err(|e| format!("Failed to create element: {e:?}"))?
            .dyn_into()
            .map_err(|_| "Failed to cast to anchor")?;

        anchor.set_href(&url);
        anchor.set_download(filename);
        anchor.click();

        // Clean up the object URL
        let _ = web_sys::Url::revoke_object_url(&url);

        Ok(())
    }
}
