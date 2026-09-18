use rustpfm::{
    ColorType, DecodeOptions, EncodeOptions, Error, ImageView, encode, read_pfm, write_pfm,
};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "rustpfm-file-test-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn files_match_byte_api_and_read_with_scale_and_limit() {
    let directory = Directory::new();
    let path = directory.0.join("image.pfm");
    let pixels = [1.0, 2.0, 3.0, 4.0];
    let view = ImageView::new(2, 2, ColorType::Gray, &pixels).unwrap();
    let options = EncodeOptions {
        scale: 2.0,
        ..EncodeOptions::default()
    };
    write_pfm(&path, view, options).unwrap();
    assert_eq!(fs::read(&path).unwrap(), encode(view, options).unwrap());
    assert_eq!(
        read_pfm(&path, DecodeOptions::default()).unwrap().pixels(),
        &[2.0, 4.0, 6.0, 8.0]
    );
    assert!(matches!(
        read_pfm(
            &path,
            DecodeOptions {
                max_pixels: Some(3),
                ..DecodeOptions::default()
            }
        ),
        Err(Error::PixelLimit)
    ));
    assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 1);
}

#[test]
fn bad_options_never_touch_destination_or_create_tempfiles() {
    let directory = Directory::new();
    let path = directory.0.join("existing.pfm");
    fs::write(&path, b"original").unwrap();
    let view = ImageView::new(1, 1, ColorType::Gray, &[1.0]).unwrap();
    for scale in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        assert!(
            write_pfm(
                &path,
                view,
                EncodeOptions {
                    scale,
                    ..EncodeOptions::default()
                }
            )
            .is_err()
        );
        assert_eq!(fs::read(&path).unwrap(), b"original");
        assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 1);
    }
}

#[test]
fn rename_failure_cleans_temporary_and_preserves_existing_directory() {
    let directory = Directory::new();
    let path = directory.0.join("existing");
    fs::create_dir(&path).unwrap();
    fs::write(path.join("keep"), b"original").unwrap();
    let view = ImageView::new(1, 1, ColorType::Gray, &[1.0]).unwrap();
    assert!(write_pfm(&path, view, EncodeOptions::default()).is_err());
    assert_eq!(fs::read(path.join("keep")).unwrap(), b"original");
    assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 1);
    assert!(
        write_pfm(
            directory.0.join("missing/image.pfm"),
            view,
            EncodeOptions::default()
        )
        .is_err()
    );
}

#[cfg(unix)]
#[test]
fn unix_permissions_and_symlink_replacement() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let directory = Directory::new();
    let path = directory.0.join("image.pfm");
    let view = ImageView::new(1, 1, ColorType::Gray, &[1.0]).unwrap();
    write_pfm(&path, view, EncodeOptions::default()).unwrap();
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
    write_pfm(&path, view, EncodeOptions::default()).unwrap();
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o640
    );
    let target = directory.0.join("target");
    fs::write(&target, b"keep target").unwrap();
    fs::set_permissions(&target, fs::Permissions::from_mode(0o644)).unwrap();
    let link = directory.0.join("link.pfm");
    symlink(&target, &link).unwrap();
    write_pfm(&link, view, EncodeOptions::default()).unwrap();
    assert!(fs::symlink_metadata(&link).unwrap().is_file());
    assert_eq!(fs::read(&target).unwrap(), b"keep target");
    assert_eq!(
        fs::metadata(&link).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let dangling = directory.0.join("dangling.pfm");
    symlink(directory.0.join("absent"), &dangling).unwrap();
    write_pfm(&dangling, view, EncodeOptions::default()).unwrap();
    assert!(fs::symlink_metadata(&dangling).unwrap().is_file());
    assert!(!directory.0.join("absent").exists());
}
