// Taken from https://github.com/rerun-io/rerun/blob/06976a36e808eae1c0879844269c7c92cc3d30d0/crates/viewer/re_ui/src/icons.rs#L128

use egui::{Image, ImageSource};

#[derive(Clone, Copy, Debug)]
pub struct Icon {
    /// Human readable unique id
    pub id: &'static str,

    pub png_bytes: &'static [u8],
}

impl Icon {
    #[inline]
    pub const fn new(id: &'static str, png_bytes: &'static [u8]) -> Self {
        Self { id, png_bytes }
    }

    #[inline]
    pub fn as_image_source(&self) -> ImageSource<'static> {
        ImageSource::Bytes {
            uri: self.id.into(),
            bytes: self.png_bytes.into(),
        }
    }

    #[inline]
    pub fn as_image(&self) -> Image<'static> {
        // Default size is the same size as the source data specifies
        const ICON_SCALE: f32 = 0.5; // Because we save all icons as 2x
        Image::new(self.as_image_source()).fit_to_original_size(ICON_SCALE)
    }
}

impl From<&'static Icon> for Image<'static> {
    #[inline]
    fn from(icon: &'static Icon) -> Self {
        icon.as_image()
    }
}

/// Macro to create an [`Icon`], using the file path as the id.
///
/// This avoids specifying the id manually, which is error-prone (duplicate IDs lead to silent
/// display bugs).
macro_rules! icon_from_path {
    ($path:literal) => {
        Icon::new($path, include_bytes!($path))
    };
}

pub const CLOSE_ICON: Icon = icon_from_path!("../../assets/close.png");
pub const EXTERNAL_LINK: Icon = icon_from_path!("../../assets/external_link.png");
pub const ICON_COLOR: Icon = icon_from_path!("../../assets/logo.png");
//pub const ICON_COLOR: Icon = icon_from_path!("../../assets/favicon.ico");
