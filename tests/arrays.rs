#![cfg(feature = "ndarray")]
use ndarray::{Array, Array2, Array3, ShapeBuilder, array, s};
use rustpfm::{ColorType, Image};
#[test]
fn borrowed_hwc_view_uses_image_buffer() {
    let image = Image::new(2, 1, ColorType::Rgb, vec![1., 2., 3., 4., 5., 6.]).unwrap();
    let view = image.as_ndarray();
    assert_eq!(view.shape(), &[1, 2, 3]);
    assert_eq!(view.as_ptr(), image.pixels().as_ptr());
    assert_eq!(view[[0, 1, 2]], 6.);
}
#[test]
fn copies_strided_and_reversed_views_in_logical_order() {
    let mut a = Array2::from_shape_fn((3, 6), |(y, x)| (y * 10 + x) as f32);
    let image = Image::from_ndarray(a.slice(s![..;-1, ..;2])).unwrap();
    assert_eq!(image.pixels(), &[20., 22., 24., 10., 12., 14., 0., 2., 4.]);
    a[[2, 0]] = 99.;
    assert_eq!(image.pixels()[0], 20.);
    assert_eq!(image.as_ndarray().shape(), &[3, 3, 1]);
}
#[test]
fn accepts_rgb_fortran_and_singleton_channels() {
    let a = Array3::from_shape_fn((2, 3, 3).f(), |(y, x, c)| (100 * y + 10 * x + c) as f32);
    let image = Image::from_ndarray(a.view()).unwrap();
    for y in 0..2 {
        for x in 0..3 {
            for c in 0..3 {
                assert_eq!(image.as_ndarray()[[y, x, c]], a[[y, x, c]]);
            }
        }
    }
    let gray = Array3::from_elem((2, 3, 1), 5.0);
    assert_eq!(
        Image::from_ndarray(gray.view()).unwrap().color_type(),
        ColorType::Gray
    );
}
#[test]
fn rejects_unsupported_dimensions_channels_and_empty_images() {
    assert!(Image::from_ndarray(array![1., 2.].view()).is_err());
    assert!(Image::from_ndarray(Array3::<f32>::zeros((2, 3, 2)).view()).is_err());
    assert!(Image::from_ndarray(Array2::<f32>::zeros((0, 3)).view()).is_err());
    assert!(Image::from_ndarray(Array::<f32, _>::zeros((1, 2, 3, 4)).view()).is_err());
}

#[test]
fn bottom_first_storage_has_top_first_negative_stride_view() {
    let mut image = Image::new(2, 3, ColorType::Gray, vec![1., 2., 3., 4., 5., 6.]).unwrap();
    image.set_row_order(rustpfm::RowOrder::BottomFirst);
    let view = image.as_ndarray();
    assert_eq!(view.strides(), &[-2, 1, 1]);
    assert_eq!(view[[0, 0, 0]], 1.);
    assert_eq!(view[[2, 1, 0]], 6.);
    assert_eq!(view.as_ptr(), image.pixels()[4..].as_ptr());
    assert_eq!(
        Image::from_ndarray(view).unwrap().pixels(),
        &[1., 2., 3., 4., 5., 6.]
    );
}
