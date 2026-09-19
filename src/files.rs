//! Filesystem conveniences with atomic replacement on supported platforms.
use std::fs::{self, File, OpenOptions};
use std::io::{BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::{DecodeOptions, EncodeOptions, Image, ImageView, Result, encode_writer};

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Read a complete PFM file, rejecting missing or trailing payload bytes.
///
/// The file size and optional pixel limit are checked before pixel allocation.
/// The file must not be modified concurrently while being read.
pub fn read_pfm<P: AsRef<Path>>(path: P, options: DecodeOptions) -> Result<Image> {
    let file = File::open(path)?;
    let total_len = file.metadata()?.len();
    crate::decode::decode_reader_sized(
        BufReader::with_capacity(64 * 1024, file),
        options,
        Some(total_len),
    )
}

/// Encode an image and atomically replace a filesystem entry.
///
/// A temporary file is created in the destination directory, then renamed after
/// successful writing and flushing. On failure, the old destination is retained
/// and temporary-file cleanup is attempted. Existing regular-file permissions
/// are copied; a symlink entry is replaced without following its target. New
/// files use mode 0600 on Unix. Ownership, ACLs and extended attributes are not
/// copied. Atomic replacement is tested on Linux; no power-loss durability is
/// promised, and concurrent changes to the destination are not coordinated.
pub fn write_pfm<P: AsRef<Path>>(
    path: P,
    view: ImageView<'_>,
    options: EncodeOptions,
) -> Result<()> {
    crate::encode::validate_options(options)?;
    write_atomic(path.as_ref(), |file| encode_writer(file, view, options))
}

struct TemporaryPath(PathBuf);
impl Drop for TemporaryPath {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn write_atomic(path: &Path, write: impl FnOnce(&mut File) -> Result<()>) -> Result<()> {
    let permissions = match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() => Some(metadata.permissions()),
        Ok(_) => None,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(error.into()),
    };
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let (temporary, mut file) = create_temporary(parent)?;
    write(&mut file)?;
    file.flush()?;
    if let Some(permissions) = permissions {
        file.set_permissions(permissions)?;
    }
    // Close before renaming, including on systems that cannot rename open files.
    drop(file);
    fs::rename(&temporary.0, path)?;
    Ok(())
}

fn create_temporary(parent: &Path) -> Result<(TemporaryPath, File)> {
    for _ in 0..128 {
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = parent.join(format!(".rustpfm-{}-{sequence}.tmp", std::process::id()));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        match options.open(&path) {
            Ok(file) => return Ok((TemporaryPath(path), file)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "could not create a unique temporary PFM file",
    )
    .into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_write_keeps_destination_and_removes_temporary() {
        let directory = std::env::temp_dir().join(format!(
            "rustpfm-write-failure-{}-{}",
            std::process::id(),
            TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&directory).unwrap();
        let destination = directory.join("image.pfm");
        fs::write(&destination, b"original").unwrap();
        let result = write_atomic(&destination, |file| {
            file.write_all(b"partial")?;
            Err(std::io::Error::other("injected write failure").into())
        });
        assert!(result.is_err());
        assert_eq!(fs::read(&destination).unwrap(), b"original");
        assert_eq!(fs::read_dir(&directory).unwrap().count(), 1);
        fs::remove_dir_all(directory).unwrap();
    }
}
