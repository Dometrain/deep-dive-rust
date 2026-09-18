/* Module 3, lesson 3.1: the C half of every FFI sample in this crate.
 *
 * Compiled and statically linked into every test binary and the app binary
 * alike by build.rs, via the `cc` crate -- see
 * `examples::module_3::linking_a_c_library` for why there is no separate
 * "linking a C library" sample beyond this file, `build.rs` and
 * `Cargo.toml`'s `[build-dependencies]` existing: that combination *is* the
 * lesson, already exercised by every other sample below. */

#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

/* examples::module_3::ffi_with_c */

int add(int a, int b) {
    return a + b;
}

void print_message(const char *message) {
    printf("%s\n", message);
}

/* examples::module_3::passing_data_to_c */

void print_rust_string(const char *str, size_t len) {
    printf("String from Rust: %.*s\n", (int)len, str);
}

typedef struct {
    const char *data;
    size_t len;
} RustString;

RustString create_string(void) {
    RustString rs;
    rs.data = "Hello from C!";
    rs.len = strlen(rs.data);
    return rs;
}

/* crate::ffi -- the applied-for-real sample: a small, fast,
 * non-cryptographic checksum used to compute the `ETag` on
 * `GET /tasks/{id}`. Never reads past `len` bytes from `data` and never
 * writes through it, so it's sound to call for any `data`/`len` pair where
 * `data` is valid for `len` reads (including `len == 0`, where `data` is
 * never dereferenced at all). */

uint32_t checksum(const uint8_t *data, size_t len) {
    uint32_t sum = 0;
    for (size_t i = 0; i < len; i++) {
        sum = (sum + (uint32_t)data[i]) * 31u;
    }
    return sum;
}
