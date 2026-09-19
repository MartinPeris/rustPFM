//! Optional ndarray interoperability.
use crate::{ColorType, Error, Image, Result, RowOrder, image::sample_count};
use ndarray::{ArrayView, ArrayView3, Dimension};

impl Image {
    /// Borrow a logical top-first height × width × channels view, without copying.
    /// Bottom-first storage is represented by a negative row stride.
    #[must_use]
    pub fn as_ndarray(&self) -> ArrayView3<'_, f32> {
        let mut view = ArrayView3::from_shape(
            (self.height(), self.width(), self.color_type().channels()),
            self.pixels(),
        )
        .expect("validated image dimensions match its sample count");
        if self.row_order() == RowOrder::BottomFirst {
            view.invert_axis(ndarray::Axis(0));
        }
        view
    }
    /// Copy a 2D grayscale or 3D HWC ndarray into owned, contiguous pixels.
    ///
    /// Channels must be 1 or 3. Strided, reversed and Fortran-order arrays are
    /// read in logical top-first order. The copy requires a full pixel buffer;
    /// encode the resulting image's view using the core APIs.
    pub fn from_ndarray<D: Dimension>(array: ArrayView<'_, f32, D>) -> Result<Self> {
        let (height, width, color) = match array.shape() {
            [h, w] | [h, w, 1] => (*h, *w, ColorType::Gray),
            [h, w, 3] => (*h, *w, ColorType::Rgb),
            _ => {
                return Err(Error::Invalid(
                    "expected a 2D grayscale or HWC array with 1 or 3 channels",
                ));
            }
        };
        let count = sample_count(width, height, color)?;
        let mut pixels = Vec::new();
        pixels
            .try_reserve_exact(count)
            .map_err(|_| Error::Allocation)?;
        pixels.extend(array.iter().copied());
        Self::new(width, height, color, pixels)
    }
}
