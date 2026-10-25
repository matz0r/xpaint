use image::RgbaImage;

/// Reads the image currently held in the system clipboard.
pub fn read_image() -> Result<RgbaImage, String> {
    let mut clipboard =
        arboard::Clipboard::new().map_err(|e| format!("Could not access clipboard: {e}"))?;

    let data = clipboard.get_image().map_err(|e| match e {
        arboard::Error::ContentNotAvailable => "The clipboard does not contain an image".to_owned(),
        e => format!("Could not read image from clipboard: {e}"),
    })?;

    RgbaImage::from_raw(
        data.width as u32,
        data.height as u32,
        data.bytes.into_owned(),
    )
    .ok_or_else(|| "Clipboard image data has an unexpected size".to_owned())
}

/// Puts images on the system clipboard.
///
/// On Linux the copying process serves the clipboard contents itself, so the
/// underlying handle is kept alive for as long as the app runs.
#[derive(Default)]
pub struct Writer {
    clipboard: Option<arboard::Clipboard>,
}

impl Writer {
    pub fn write_image(&mut self, image: &RgbaImage) -> Result<(), String> {
        let clipboard = match &mut self.clipboard {
            Some(clipboard) => clipboard,
            None => self.clipboard.insert(
                arboard::Clipboard::new()
                    .map_err(|e| format!("Could not access clipboard: {e}"))?,
            ),
        };
        clipboard
            .set_image(arboard::ImageData {
                width: image.width() as usize,
                height: image.height() as usize,
                bytes: image.as_raw().into(),
            })
            .map_err(|e| format!("Could not copy image to clipboard: {e}"))
    }
}
