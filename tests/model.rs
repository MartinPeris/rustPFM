use rustpfm::{ColorType, Error, Image, ImageView};
use std::error::Error as _;

#[test]
fn image_ownership_and_validated_views() {
    let mut image = Image::new(2, 1, ColorType::Rgb, vec![0.; 6]).unwrap();
    image.pixels_mut()[5] = 1.;
    let view = image.view();
    assert_eq!(
        (view.width(), view.height(), view.color_type()),
        (2, 1, ColorType::Rgb)
    );
    assert_eq!(view.pixels()[5], 1.);
    let cloned = image.clone();
    assert_eq!(cloned.into_pixels(), image.into_pixels());
    for (width, height, color, length) in [
        (0, 1, ColorType::Gray, 0),
        (1, 0, ColorType::Gray, 0),
        (1, 1, ColorType::Rgb, 2),
        (usize::MAX, 2, ColorType::Gray, 0),
        (usize::MAX / 4, 1, ColorType::Gray, 0),
    ] {
        assert!(ImageView::new(width, height, color, &vec![0.; length]).is_err());
    }
}

#[test]
fn errors_are_descriptive_and_preserve_io_source() {
    for error in [
        Error::Invalid("test"),
        Error::SizeOverflow,
        Error::PixelLimit,
        Error::Allocation,
    ] {
        assert!(!error.to_string().is_empty());
        assert!(error.source().is_none());
    }
    let error = Error::from(std::io::Error::other("injected failure"));
    assert!(error.to_string().contains("injected failure"));
    assert!(error.source().is_some());
}
