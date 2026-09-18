//! The one `unsafe` block that ships in this app's production code path --
//! Module 3's raw-pointer/FFI lesson
//! (`examples::module_3::{ffi_with_c, passing_data_to_c}`), applied for real
//! instead of only as a standalone sample.
//!
//! See the README section "why raw pointers and `unsafe` never appear in
//! `src/routes` or `src/main.rs`" for why this stays this narrow: one safe
//! wrapper function, around one `extern "C"` call, to a C function whose own
//! signature has no pointers a caller could get wrong -- `checksum_hex` is
//! the only place in this module that has to reason about pointer validity
//! at all, and it does so once.

use std::os::raw::c_uchar;

extern "C" {
    fn checksum(data: *const c_uchar, len: usize) -> u32;
}

/// A fast, non-cryptographic checksum of `bytes`, formatted as 8 lowercase
/// hex digits. Used as the `ETag` on `GET /tasks/{id}` (see
/// [`crate::models::task::Task::etag`]) so a client can cheaply tell "did
/// this task change since I last fetched it?" without comparing the whole
/// response body.
///
/// Deliberately *not* cryptographic: nothing here needs to resist a
/// deliberate collision attempt, only to change whenever the task's fields
/// do -- reaching for a hash like SHA-256 here would be paying for a
/// guarantee this job doesn't need.
pub fn checksum_hex(bytes: &[u8]) -> String {
    // SAFETY: `bytes` is a live `&[u8]` borrow for the whole call, so
    // `bytes.as_ptr()` is valid for `bytes.len()` reads until `checksum_hex`
    // returns. `native/native.c`'s `checksum` only ever reads -- never
    // writes -- at most `len` bytes starting from `data`, and never
    // dereferences `data` at all when `len == 0` (see its loop condition),
    // which is what makes calling it sound even for an empty slice, whose
    // `as_ptr()` may be a dangling-but-non-null, aligned pointer rather than
    // a pointer to a real allocation.
    let sum = unsafe { checksum(bytes.as_ptr(), bytes.len()) };
    format!("{sum:08x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_deterministic() {
        assert_eq!(checksum_hex(b"Learn Rust"), checksum_hex(b"Learn Rust"));
    }

    #[test]
    fn differs_for_different_input() {
        assert_ne!(checksum_hex(b"Learn Rust"), checksum_hex(b"Learn Rust!"));
    }

    #[test]
    fn handles_an_empty_slice_without_dereferencing_a_dangling_pointer() {
        assert_eq!(checksum_hex(b""), checksum_hex(&[]));
    }
}
