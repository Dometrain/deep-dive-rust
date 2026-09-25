//! Standalone teaching snippets for *Deep Dive: Rust* Module 3 ("Unsafe
//! Rust"), one submodule per code sample in the lesson -- same structure as
//! [`crate::examples::module_2`].
//!
//! The C functions the FFI-flavoured samples below call all live in one
//! file, [`native/native.c`](../../../native/native.c), compiled and
//! statically linked into this crate by `build.rs` via the `cc` crate --
//! once, at build time, before any of the tests below (or `crate::ffi`, or
//! `cargo run`) exist as a binary. See `linking_a_c_library` for why that
//! makes it this module's lesson too, not a separate sample.
//!
//! The *applied* version of this lesson lives in the real HTTP API: see
//! [`crate::ffi::checksum_hex`], the one `unsafe` block that ships outside
//! this `examples` module, and [`crate::models::task::Task::etag`], which
//! calls it.

/// 3.1 #1 -- Raw Pointer Basics.
pub mod raw_pointer_basics {
    /// Creates a `*const i32` and a `*mut i32` from the same value and reads
    /// through both, then writes through the mutable one.
    ///
    /// Neither pointer is checked by the borrow checker: `raw_ptr_const` and
    /// `raw_ptr_mut` are allowed to coexist even though this is exactly the
    /// "shared and mutable at once" shape that ordinary `&x` / `&mut x`
    /// references would refuse to compile if `raw_ptr_const`'s value were
    /// read again after `raw_ptr_mut` was created. Raw pointers don't lift
    /// that rule -- they just stop the compiler from checking it for you.
    /// See `unsafe_functions` and `dereferencing_raw_pointers` below for
    /// what you're on the hook for instead.
    pub fn read_and_write(initial: i32, new_value: i32) -> (i32, i32) {
        let mut x = initial;

        let raw_ptr_const: *const i32 = &x as *const i32;
        let raw_ptr_mut: *mut i32 = &mut x as *mut i32;

        unsafe {
            let before = *raw_ptr_const;
            *raw_ptr_mut = new_value;
            (before, *raw_ptr_mut)
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn reads_the_original_then_the_modified_value() {
            assert_eq!(read_and_write(10, 20), (10, 20));
        }
    }
}

/// 3.1 #2 -- Dereferencing Raw Pointers.
pub mod dereferencing_raw_pointers {
    /// Writes through a raw pointer and reads the result back. This is the
    /// safe case: the pointee (`x`) is alive and reachable for the entire
    /// `unsafe` block.
    pub fn write_and_read_back(initial: i32, new_value: i32) -> i32 {
        let mut x = initial;
        let raw_ptr: *mut i32 = &mut x as *mut i32;

        // SAFETY: `raw_ptr` was created from `&mut x` one line above, and
        // `x` is not dropped, moved, or aliased anywhere in this function.
        unsafe {
            *raw_ptr = new_value;
            *raw_ptr
        }
    }

    /// Returns a raw pointer to a value that is about to go out of scope --
    /// this **compiles**, which is the whole point. With a `&i32` instead of
    /// a `*const i32`, the borrow checker would reject this function outright
    /// ("returns a value referencing data owned by the current function").
    /// Raw pointers carry no such lifetime, so the compiler lets it through
    /// silently; the returned pointer is dangling the moment this function
    /// returns.
    ///
    /// This function is never dereferenced anywhere in this codebase, on
    /// purpose -- doing so is undefined behavior, not a panic, and this
    /// module would rather show you the pointer than the crash.
    ///
    /// `#[allow(...)]` below is silencing a real compiler lint,
    /// `dangling_pointers_from_locals` -- recent rustc versions *do* notice
    /// this specific mistake (a raw pointer to a local, returned past the
    /// local's scope) and warn about it by default. That lint is a static
    /// analysis heuristic, not the borrow checker -- it doesn't catch every
    /// way to produce a dangling pointer (see the module doc's example in
    /// `unsafe_functions` for one shape it can't see), which is exactly why
    /// `unsafe` still puts the burden on you rather than the compiler.
    #[allow(dangling_pointers_from_locals)]
    pub fn dangling_pointer() -> *const i32 {
        let x = 42;
        &x as *const i32
        // `x`'s stack slot is freed here. The pointer returned above still
        // holds its address.
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn writes_the_new_value_through_the_pointer() {
            assert_eq!(write_and_read_back(1, 2), 2);
        }

        #[test]
        fn dangling_pointer_compiles_but_this_test_never_dereferences_it() {
            // Proving the point, not the crash: creating the pointer
            // compiles (unlike the `&i32` equivalent would), and this test
            // deliberately stops before ever writing `unsafe { *ptr }`.
            let _ptr = dangling_pointer();
        }
    }
}

/// 3.1 #3 -- `unsafe` Functions.
///
/// **Honesty check, in the same spirit as `module_2::variance`'s:** integer
/// division by zero in Rust already panics safely, with no `unsafe`
/// involved -- it isn't actually a memory-safety hazard. `dangerous_divide`
/// doesn't need `unsafe fn` to protect against div-by-zero specifically; it
/// exists here (matching the lesson) purely to show the *syntax* of an
/// `unsafe fn` and the caller-must-uphold-an-invariant contract that comes
/// with it. For a sample where the invariant genuinely guards against memory
/// corruption or reading freed memory, see
/// `dereferencing_raw_pointers::dangling_pointer` above.
pub mod unsafe_functions {
    /// # Safety
    ///
    /// `denominator` must be non-zero. Callers upholding that invariant
    /// (like `safe_divide` below) is what makes calling this function sound.
    pub unsafe fn dangerous_divide(numerator: i32, denominator: i32) -> i32 {
        numerator / denominator
    }

    pub fn safe_divide(numerator: i32, denominator: i32) -> Option<i32> {
        if denominator == 0 {
            None
        } else {
            // SAFETY: just checked `denominator != 0` above -- the
            // precondition `dangerous_divide` documents is upheld.
            Some(unsafe { dangerous_divide(numerator, denominator) })
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn divides_when_the_denominator_is_non_zero() {
            assert_eq!(safe_divide(10, 2), Some(5));
        }

        #[test]
        fn returns_none_instead_of_panicking_on_zero() {
            assert_eq!(safe_divide(10, 0), None);
        }
    }
}

/// 3.1 #4 -- FFI with C.
pub mod ffi_with_c {
    use std::ffi::CString;
    use std::os::raw::c_char;

    extern "C" {
        fn add(a: i32, b: i32) -> i32;
        fn print_message(message: *const c_char);
    }

    pub fn add_via_c(a: i32, b: i32) -> i32 {
        // SAFETY: `add` (see `native/native.c`) is a pure function over two
        // `i32`s -- no pointers in its signature, no shared state, and it
        // cannot panic or trap for any input.
        unsafe { add(a, b) }
    }

    /// Prints `message` from C, via `printf`.
    ///
    /// Uses `CString`, not a byte-string literal cast straight to a pointer
    /// -- `print_message`'s C signature (`const char *`) requires a
    /// **null-terminated** buffer, and a Rust byte string like `b"hi"`
    /// is *not* null-terminated on its own. `CString::new` is what actually
    /// appends the trailing `\0` (and rejects a message that already
    /// contains one in the middle, which C's `%s` would otherwise silently
    /// truncate at).
    pub fn print_via_c(message: &str) {
        let c_message = CString::new(message).expect("message must not contain a NUL byte");

        // SAFETY: `c_message` is not dropped until this function returns, so
        // the pointer stays valid for the whole call, and `CString::new`
        // guarantees the buffer it hands back is null-terminated -- exactly
        // what `print_message`'s `const char *` contract requires.
        unsafe { print_message(c_message.as_ptr()) };
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn adds_two_numbers_via_c() {
            assert_eq!(add_via_c(5, 3), 8);
        }

        #[test]
        fn prints_without_panicking() {
            print_via_c("Hello from C!");
        }
    }
}

/// 3.1 #5 -- Passing Data to C.
pub mod passing_data_to_c {
    use std::ffi::CString;
    use std::os::raw::c_char;

    // `RustString` is declared *outside* the `extern "C"` block -- a struct
    // definition (even one with `#[repr(C)]`) isn't an external item, only
    // the function that returns one is.
    #[repr(C)]
    struct RustString {
        data: *const c_char,
        len: usize,
    }

    extern "C" {
        fn print_rust_string(str: *const c_char, len: usize);
        fn create_string() -> RustString;
    }

    /// Sends a Rust `&str` to C and lets it print it.
    pub fn send_to_c(s: &str) {
        let c_string = CString::new(s).expect("message must not contain a NUL byte");

        // SAFETY: `c_string` is a valid pointer for the whole call (it isn't
        // dropped until this function returns), and `s.len()` -- the byte
        // length of the *original* Rust string, not including the `\0`
        // `CString` appended -- is exactly how many bytes `print_rust_string`
        // reads via its `%.*s` format.
        unsafe {
            print_rust_string(c_string.as_ptr(), s.len());
        }
    }

    /// Receives a string from C and converts it into an owned Rust `String`.
    pub fn receive_from_c() -> String {
        // SAFETY: `create_string` (see `native/native.c`) always returns a
        // `RustString` pointing at a live, valid buffer of exactly `len`
        // bytes -- it's a pointer into a C string literal with static
        // storage duration, not something that could have already been
        // freed.
        let rust_string = unsafe { create_string() };

        // SAFETY: `rust_string.data` is valid for `rust_string.len` reads
        // (guaranteed by `create_string`'s contract above), and that memory
        // is never mutated for as long as this slice exists.
        let slice =
            unsafe { std::slice::from_raw_parts(rust_string.data as *const u8, rust_string.len) };
        std::str::from_utf8(slice)
            .expect("native/native.c always returns UTF-8")
            .to_owned()
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn sends_a_string_without_panicking() {
            send_to_c("Hello from Rust!");
        }

        #[test]
        fn receives_the_c_strings_contents() {
            assert_eq!(receive_from_c(), "Hello from C!");
        }
    }
}

/// 3.1 #6 -- Linking a C Library.
///
/// No new code here on purpose: every test in `ffi_with_c`,
/// `passing_data_to_c` and `crate::ffi` already *is* this lesson.
/// [`native/native.c`](../../../native/native.c) is a small C library;
/// `build.rs` uses the `cc` crate (declared under `[build-dependencies]` in
/// `Cargo.toml`) to compile it and statically link it into this exact
/// binary before a single line of Rust runs. If that link step ever failed,
/// none of the tests in this file -- or `crate::ffi`'s -- would compile,
/// let alone pass. There's no separate "and now link it" step left to
/// demonstrate, because it already had to happen for everything above this
/// line to exist.
pub mod linking_a_c_library {
    #[cfg(test)]
    mod tests {
        use crate::examples::module_3::ffi_with_c::add_via_c;

        #[test]
        fn the_static_library_is_already_linked_in() {
            // If `native/native.c` weren't compiled and linked by
            // `build.rs`, this call wouldn't link, and `cargo test`
            // wouldn't produce a binary to run in the first place.
            assert_eq!(add_via_c(2, 3), 5);
        }
    }
}

/// 3.2 -- Macros: the compiler's *other* "do it yourself" feature.
///
/// Where `unsafe` (3.1) lets you step outside the compiler's safety checks,
/// macros let you step into its **code generation** -- everything from
/// `println!` to `#[derive(Serialize)]` is a macro, code written for you at
/// compile time. There are three kinds; this sample shows the two you can
/// write and test standalone (declarative and a derive-*use*), and documents
/// the third (attribute).
pub mod macros {
    /// 3.2 #1 -- A declarative macro (`macro_rules!`). A miniature `vec!`:
    /// matches a comma-separated list (`$( $item:expr ),*`) and expands to a
    /// `Vec` built by `push`ing each one. This is the same machinery behind
    /// `vec!`, `println!`, and `assert_eq!`.
    #[macro_export]
    macro_rules! my_vec {
        ( $( $item:expr ),* $(,)? ) => {{
            let mut v = Vec::new();
            $( v.push($item); )*   // the body repeats once per matched item
            v
        }};
    }

    /// 3.2 #2 -- What `#[derive(...)]` generates. A *derive macro* is a
    /// procedural macro: it reads a type's tokens and emits an `impl`. The
    /// `#[derive(Default, Debug, Clone, PartialEq)]` below generates, field by
    /// field, code you could have written by hand -- e.g. `Default` produces
    /// roughly:
    ///
    /// ```ignore
    /// impl Default for Config {
    ///     fn default() -> Self {
    ///         Config { retries: u32::default(), verbose: bool::default() }
    ///     }
    /// }
    /// ```
    ///
    /// The headline for a .NET audience: this is compile-time code generation
    /// (like a source generator), *not* the runtime reflection
    /// `System.Text.Json` uses -- so a missing impl is a compile error, not a
    /// runtime exception.
    #[derive(Default, Debug, Clone, PartialEq)]
    pub struct Config {
        pub retries: u32,
        pub verbose: bool,
    }

    // 3.2 #3 -- Attribute macros (documented, not shown as a standalone test:
    // they need the runtime they wrap). `#[tokio::main]` rewrites an
    // `async fn main` into a normal `fn main` that starts a runtime;
    // `#[tracing::instrument]` (Module 5) wraps a function body in a span.
    // Three kinds in one line each: declarative (tokens -> tokens), derive
    // (generates an impl from a type), attribute (rewrites the item it's on).

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn a_declarative_macro_expands_to_a_vec() {
            let nums = my_vec![1, 2, 3]; // expands to push(1); push(2); push(3);
            assert_eq!(nums, vec![1, 2, 3]);
            let empty: Vec<i32> = my_vec![];
            assert!(empty.is_empty());
        }

        #[test]
        fn derived_impls_are_generated_code_you_never_wrote() {
            // `Default`, `Debug`, `Clone`, `PartialEq` all came from the
            // derive -- none of these methods were written by hand.
            let a = Config::default(); // from #[derive(Default)]
            assert_eq!(a.retries, 0);
            assert!(!a.verbose);

            let b = a.clone(); // from #[derive(Clone)]
            assert_eq!(a, b); // from #[derive(PartialEq)]
            assert_eq!(format!("{b:?}"), "Config { retries: 0, verbose: false }");
            // Debug
        }
    }
}
