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
/// Physical storage order. Logical row access always counts from the top.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RowOrder {
    /// Contiguous top-first storage, convenient for conventional image buffers.
    #[default]
    TopFirst,
    /// PFM file order; avoids reversing rows after decoding.
    BottomFirst,
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
    /// Physical row order of the returned buffer. Defaults to top-first.
    pub row_order: RowOrder,
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
/// Contiguous interleaved float32 storage with explicit physical row order.
#[derive(Clone, Debug)]
pub struct Image {
    header: Header,
    pixels: Vec<f32>,
    row_order: RowOrder,
}
impl Image {
    pub(crate) fn from_decoded(header: Header, pixels: Vec<f32>, row_order: RowOrder) -> Self {
        Self {
            header,
            pixels,
            row_order,
        }
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
            row_order: RowOrder::TopFirst,
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
    /// Borrow native-endian samples in physical `row_order()` (not always top-first).
    pub fn pixels(&self) -> &[f32] {
        &self.pixels
    }
    /// Mutably borrow samples without changing image dimensions.
    pub fn pixels_mut(&mut self) -> &mut [f32] {
        &mut self.pixels
    }
    /// Consume the image and return its buffer in physical `row_order()`.
    pub fn into_pixels(self) -> Vec<f32> {
        self.pixels
    }
    /// Physical row order of the pixel buffer.
    pub fn row_order(&self) -> RowOrder {
        self.row_order
    }
    /// Borrow a logical row counted from the top, or None when out of bounds.
    pub fn row(&self, y: usize) -> Option<&[f32]> {
        self.view().row(y)
    }
    /// Reorder storage in place without changing logical pixel positions.
    pub fn set_row_order(&mut self, order: RowOrder) {
        if self.row_order == order {
            return;
        }
        let row = self.width() * self.color_type().channels();
        let height = self.height();
        let (top, rest) = self.pixels.split_at_mut((height / 2) * row);
        let bottom = &mut rest[(height % 2) * row..];
        for (a, b) in top
            .chunks_exact_mut(row)
            .zip(bottom.chunks_exact_mut(row).rev())
        {
            a.swap_with_slice(b);
        }
        self.row_order = order;
    }
    /// Borrow a validated view suitable for encoding.
    pub fn view(&self) -> ImageView<'_> {
        ImageView {
            width: self.width(),
            height: self.height(),
            color_type: self.color_type(),
            pixels: &self.pixels,
            row_order: self.row_order,
        }
    }
}
/// A validated borrowed contiguous pixel buffer with explicit physical row order.
#[derive(Clone, Copy, Debug)]
pub struct ImageView<'a> {
    width: usize,
    height: usize,
    color_type: ColorType,
    pixels: &'a [f32],
    row_order: RowOrder,
}
impl<'a> ImageView<'a> {
    /// Validate dimensions and exact sample count without copying pixels.
    pub fn new(
        width: usize,
        height: usize,
        color_type: ColorType,
        pixels: &'a [f32],
    ) -> Result<Self> {
        Self::with_row_order(width, height, color_type, pixels, RowOrder::TopFirst)
    }
    /// Validate a buffer whose physical row order is specified explicitly.
    pub fn with_row_order(
        width: usize,
        height: usize,
        color_type: ColorType,
        pixels: &'a [f32],
        row_order: RowOrder,
    ) -> Result<Self> {
        if sample_count(width, height, color_type)? != pixels.len() {
            return Err(Error::Invalid("sample count does not match dimensions"));
        }
        Ok(Self {
            width,
            height,
            color_type,
            pixels,
            row_order,
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
    /// Physical row order of the sample buffer.
    pub fn row_order(self) -> RowOrder {
        self.row_order
    }
    /// Borrow a logical row counted from the top, or None when out of bounds.
    pub fn row(self, y: usize) -> Option<&'a [f32]> {
        if y >= self.height {
            return None;
        }
        let physical = match self.row_order {
            RowOrder::TopFirst => y,
            RowOrder::BottomFirst => self.height - 1 - y,
        };
        let width = self.width * self.color_type.channels();
        Some(&self.pixels[physical * width..(physical + 1) * width])
    }
    pub(crate) fn file_rows(self) -> impl Iterator<Item = &'a [f32]> {
        (0..self.height)
            .rev()
            .map(move |y| self.row(y).expect("row index is in bounds"))
    }
    /// Borrow all samples in physical `row_order()`.
    pub fn pixels(self) -> &'a [f32] {
        self.pixels
    }
}
