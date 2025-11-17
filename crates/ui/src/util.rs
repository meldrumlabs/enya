pub use egui::SizeHint;
use egui::epaint;
pub use egui_extras::image::{load_svg_bytes, load_svg_bytes_with_size};

pub fn color_image_to_icon_data(image: epaint::ColorImage) -> egui::IconData {
    egui::IconData {
        width: image.size[0] as u32,
        height: image.size[1] as u32,
        rgba: image.as_raw().to_vec(),
    }
}

pub fn svg_to_icon_data(svg_bytes: &[u8], size_hint: Option<SizeHint>) -> egui::IconData {
    let image = load_svg_bytes_with_size(svg_bytes, size_hint).unwrap();
    color_image_to_icon_data(image)
}
pub fn png_to_icon_data(png_bytes: &[u8]) -> egui::IconData {
    let image = image::load_from_memory(png_bytes).unwrap();
    let size = [image.width() as usize, image.height() as usize];
    let rgba = image.into_rgba8().to_vec();
    egui::IconData {
        width: size[0] as u32,
        height: size[1] as u32,
        rgba,
    }
}
