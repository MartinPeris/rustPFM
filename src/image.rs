//! Validated owned images and borrowed pixel views.
use crate::{Error, Result};

/// Number and interpretation of interleaved channels.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ColorType {
    /// One grayscale float per pixel.
    Gray,
    /// Red, green and blue floats per pixel.
    Rgb,
}
impl ColorType {
    /// Number of float samples per pixel.
    pub const fn channels(self) -> usize {
        match self {
            Self::Gray => 1,
            Self::Rgb => 3,
        }
    }
}
/// Byte order of the samples stored in a PFM payload.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ByteOrder {
    /// Least significant byte first.
    #[default]
    Little,
    /// Most significant byte first.
    Big,
}
/// Header metadata. Pixels held by an image are always native-endian floats.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Header {
    /// Width in pixels.
    pub width: usize,
    /// Height in pixels.
    pub height: usize,
    /// Grayscale or interleaved RGB.
    pub color_type: ColorType,
    /// Positive finite float32 scale magnitude stored in the source header.
    pub scale: f32,
    /// Source payload byte order.
    pub byte_order: ByteOrder,
}
/// Interpretation of the header scale while decoding.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ScaleMode {
    /// Multiply samples by the header magnitude (the justPFM convention).
    #[default]
    Apply,
    /// Return original samples without scaling.
    Raw,
}
/// Decode options. A pixel limit counts width × height, not channel samples.
#[derive(Clone, Copy, Debug, Default)]
pub struct DecodeOptions {
    /// Positive maximum pixel count; `None` permits any representable size.
    pub max_pixels: Option<usize>,
    /// Whether to apply the header scale.
    pub scale_mode: ScaleMode,
}
/// Encode options. Samples are stored unchanged; scale is header metadata.
#[derive(Clone, Copy, Debug)]
pub struct EncodeOptions {
    /// Positive finite float32 scale magnitude.
    pub scale: f32,
    /// Output payload byte order.
    pub byte_order: ByteOrder,
}
impl Default for EncodeOptions {
    fn default() -> Self {
        Self {
            scale: 1.0,
            byte_order: ByteOrder::Little,
        }
    }
}
pub(crate) fn sample_count(width: usize, height: usize, color: ColorType) -> Result<usize> {
    if width == 0 || height == 0 {
        return Err(Error::Invalid("dimensions must be positive"));
    }
    let count = width
        .checked_mul(height)
        .and_then(|n| n.checked_mul(color.channels()))
        .ok_or(Error::SizeOverflow)?;
    let bytes = count.checked_mul(4).ok_or(Error::SizeOverflow)?;
    if bytes > isize::MAX as usize {
        return Err(Error::SizeOverflow);
    }
    Ok(count)
}
/// Contiguous top-first, interleaved float32 pixels owned by the caller.
#[derive(Clone, Debug)]
pub struct Image {
    header: Header,
    pixels: Vec<f32>,
}
impl Image {
    pub(crate) fn from_decoded(header: Header, pixels: Vec<f32>) -> Self {
        Self { header, pixels }
    }
    /// Validate dimensions and exact sample count without copying the vector.
    pub fn new(
        width: usize,
        height: usize,
        color_type: ColorType,
        pixels: Vec<f32>,
    ) -> Result<Self> {
        ImageView::new(width, height, color_type, &pixels)?;
        Ok(Self {
            header: Header {
                width,
                height,
                color_type,
                scale: 1.0,
                byte_order: ByteOrder::Little,
            },
            pixels,
        })
    }
    /// Source header, including original scale even after scaled decoding.
    pub fn header(&self) -> Header {
        self.header
    }
    /// Width in pixels.
    pub fn width(&self) -> usize {
        self.header.width
    }
    /// Height in pixels.
    pub fn height(&self) -> usize {
        self.header.height
    }
    /// Channel interpretation.
    pub fn color_type(&self) -> ColorType {
        self.header.color_type
    }
    /// Borrow top-first native-endian samples.
    pub fn pixels(&self) -> &[f32] {
        &self.pixels
    }
    /// Mutably borrow samples without changing image dimensions.
    pub fn pixels_mut(&mut self) -> &mut [f32] {
        &mut self.pixels
    }
    /// Consume the image and return its pixel buffer.
    pub fn into_pixels(self) -> Vec<f32> {
        self.pixels
    }
    /// Borrow a validated view suitable for encoding.
    pub fn view(&self) -> ImageView<'_> {
        ImageView {
            width: self.width(),
            height: self.height(),
            color_type: self.color_type(),
            pixels: &self.pixels,
        }
    }
}
/// A validated borrowed, contiguous, top-first pixel buffer.
#[derive(Clone, Copy, Debug)]
pub struct ImageView<'a> {
    width: usize,
    height: usize,
    color_type: ColorType,
    pixels: &'a [f32],
}
impl<'a> ImageView<'a> {
    /// Validate dimensions and exact sample count without copying pixels.
    pub fn new(
        width: usize,
        height: usize,
        color_type: ColorType,
        pixels: &'a [f32],
    ) -> Result<Self> {
        if sample_count(width, height, color_type)? != pixels.len() {
            return Err(Error::Invalid("sample count does not match dimensions"));
        }
        Ok(Self {
            width,
            height,
            color_type,
            pixels,
        })
    }
    /// Width in pixels.
    pub fn width(self) -> usize {
        self.width
    }
    /// Height in pixels.
    pub fn height(self) -> usize {
        self.height
    }
    /// Channel interpretation.
    pub fn color_type(self) -> ColorType {
        self.color_type
    }
    /// Borrow all samples.
    pub fn pixels(self) -> &'a [f32] {
        self.pixels
    }
}
