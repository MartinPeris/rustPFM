//! The only unsafe code: allocation, initialized f32 byte views, and Linux hints.
use crate::{Error, Result};
use std::{
    alloc::{Layout, alloc_zeroed},
    slice,
};

pub(crate) fn zeroed(count: usize) -> Result<Vec<f32>> {
    if count == 0 {
        return Ok(Vec::new());
    }
    let layout = Layout::array::<f32>(count).map_err(|_| Error::SizeOverflow)?;
    // SAFETY: count is nonzero and Layout::array checked size/alignment.
    // The global allocator returns either null or a suitably aligned, zeroed
    // allocation. Zero bytes are a valid initialized f32 representation.
    let pointer = unsafe { alloc_zeroed(layout) }.cast::<f32>();
    if pointer.is_null() {
        return Err(Error::Allocation);
    }
    #[cfg(all(feature = "hugepages", target_os = "linux", not(miri)))]
    // SAFETY: the nonnull allocation is live, uniquely owned, and has layout.size() bytes.
    unsafe {
        advise_hugepages(pointer.cast(), layout.size());
    }
    // SAFETY: allocation uses the global allocator and exactly the layout of
    // `count` f32s. All elements are initialized; length equals capacity. Unique
    // ownership is transferred once to Vec, which uses the matching deallocator.
    Ok(unsafe { Vec::from_raw_parts(pointer, count, count) })
}

pub(crate) fn bytes(samples: &[f32]) -> &[u8] {
    // SAFETY: f32 has no padding; every input element is initialized. Its bytes
    // occupy this same allocation, and u8 has alignment 1. The byte length fits
    // isize because this is an existing valid f32 slice. The lifetime is tied
    // to the shared input borrow and no mutation is possible through this view.
    unsafe { slice::from_raw_parts(samples.as_ptr().cast(), std::mem::size_of_val(samples)) }
}

pub(crate) fn bytes_mut(samples: &mut [f32]) -> &mut [u8] {
    // SAFETY: as above, with an exclusive borrow lasting for the returned view.
    // Every f32 bit pattern is valid, so arbitrary writes, partial reads, I/O
    // errors and unwinding cannot leave invalid f32 values. All bytes were
    // already initialized: an arbitrary safe Read may inspect them before writing.
    unsafe {
        slice::from_raw_parts_mut(samples.as_mut_ptr().cast(), std::mem::size_of_val(samples))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn allocation_and_byte_views_preserve_all_bits() {
        assert!(zeroed(0).unwrap().is_empty());
        assert!(matches!(zeroed(usize::MAX), Err(Error::SizeOverflow)));
        let words = [0, 0x8000_0000, 0x7f80_0001, 0x7fc0_1234, 0xffff_ffff];
        let mut values = zeroed(words.len()).unwrap();
        assert!(bytes(&values).iter().all(|&byte| byte == 0));
        for (chunk, word) in bytes_mut(&mut values).chunks_exact_mut(4).zip(words) {
            chunk.copy_from_slice(&u32::to_ne_bytes(word));
        }
        assert_eq!(
            values.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
            words
        );
        assert!(bytes(&[]).is_empty());
        assert!(bytes_mut(&mut []).is_empty());
        // Vec's ownership/deallocation remains valid after growth and cloning.
        values.push(1.0);
        assert_eq!(values.clone().last(), Some(&1.0));
    }
}

/// Hint only full pages strictly inside this owned allocation. Failure is benign.
#[cfg(all(feature = "hugepages", target_os = "linux", not(miri)))]
unsafe fn advise_hugepages(pointer: *mut u8, length: usize) {
    // Linux ABI: MADV_HUGEPAGE is 14. getpagesize avoids assuming 4 KiB pages.
    // Both functions are supplied by the platform C library; no Rust dependency.
    unsafe extern "C" {
        fn getpagesize() -> std::ffi::c_int;
        fn madvise(
            address: *mut std::ffi::c_void,
            length: usize,
            advice: std::ffi::c_int,
        ) -> std::ffi::c_int;
    }
    if length < 2 * 1024 * 1024 {
        return;
    }
    // SAFETY: getpagesize has no pointer arguments or caller preconditions.
    let page = unsafe { getpagesize() };
    if page <= 0 {
        return;
    }
    let Some((offset, span)) = page_region(pointer.addr(), length, page as usize) else {
        return;
    };
    // SAFETY: offset/span select only complete mapped pages inside the live,
    // uniquely owned allocation; pointer arithmetic stays in that allocation.
    // MADV_HUGEPAGE is a nondestructive hint, unlike DONTNEED/FREE. Its result
    // cannot affect ownership or initialized-byte validity and is ignored.
    unsafe {
        let _ = madvise(pointer.add(offset).cast(), span, 14);
    }
}

#[cfg(all(feature = "hugepages", target_os = "linux"))]
#[cfg_attr(miri, allow(dead_code))]
fn page_region(address: usize, length: usize, page: usize) -> Option<(usize, usize)> {
    if page == 0 {
        return None;
    }
    let offset = (page - address % page) % page;
    let span = length.checked_sub(offset)? / page * page;
    if span == 0 {
        None
    } else {
        Some((offset, span))
    }
}

#[cfg(all(test, feature = "hugepages", target_os = "linux"))]
mod page_tests {
    #[test]
    fn range_never_includes_partial_or_foreign_pages() {
        for page in [4096, 16384, 65536] {
            assert_eq!(super::page_region(0, page * 3, page), Some((0, page * 3)));
            assert_eq!(
                super::page_region(1, page * 3, page),
                Some((page - 1, page * 2))
            );
            assert_eq!(
                super::page_region(page - 1, page + 1, page),
                Some((1, page))
            );
            assert_eq!(super::page_region(1, 1, page), None);
            assert_eq!(super::page_region(0, page - 1, page), None);
        }
        assert_eq!(super::page_region(0, 1, 0), None);
        assert_eq!(super::page_region(usize::MAX, 1, 4096), None);
    }
    #[test]
    #[cfg(not(miri))]
    fn native_large_allocation_remains_zeroed() {
        let mut samples = super::zeroed(1024 * 1024).unwrap();
        assert!(super::bytes(&samples).iter().all(|&byte| byte == 0));
        samples[0] = -0.0;
        samples[1024 * 1024 - 1] = 42.0;
        assert_eq!(samples[0].to_bits(), (-0.0_f32).to_bits());
        assert_eq!(samples.last(), Some(&42.0));
    }
}
