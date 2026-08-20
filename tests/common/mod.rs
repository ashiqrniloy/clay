//! Shared source-policy helpers for the editor performance + Rust visibility
//! source-text assertion tests. Centralizes the recurring read / non-test-slice
//! / absence / concat / visibility-mapping patterns so each test states its
//! file list + needle vocabulary without re-deriving the diagnostic plumbing.
//! Source-text contracts (files, needles) are preserved verbatim; only the
//! boilerplate is shared. `tests/common/mod.rs`, included via `mod common;`.

#![allow(dead_code)] // each test crate uses a different subset of these helpers

use std::fs;

fn full(path: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(path)
}

/// Read a source file with a clear panic naming the path on failure.
pub fn read_src(path: &str) -> String {
    fs::read_to_string(full(path)).unwrap_or_else(|e| panic!("read {path}: {e}"))
}

/// Slice off `#[cfg(test)] mod tests` (and trailing test-only imports) so
/// source-policy scans see production code only.
pub fn non_test(src: &str) -> &str {
    if let Some(i) = src.find("\nmod tests") {
        return &src[..i];
    }
    if let Some(i) = src.find("\n#[cfg(test)]") {
        return &src[..i];
    }
    src
}

/// Concatenate the non-test bodies of `files` (newline-separated) for a single
/// absence scan across a hot-path file set.
pub fn hot_path_concat(files: &[&str]) -> String {
    let mut out = String::new();
    for &path in files {
        out.push_str(non_test(&read_src(path)));
        out.push('\n');
    }
    out
}

/// Assert no needle in `needles` appears in `body`. Centralizes the recurring
/// `for forbidden in [...] { assert!(!body.contains(forbidden), ...) }` loop.
pub fn assert_absent(body: &str, needles: &[&str], label: &str) {
    for &n in needles {
        assert!(!body.contains(n), "{label}: must not contain {n:?}");
    }
}

/// Assert each `(path, needle)` pair: the file's non-test body contains the
/// needle. Centralizes the visibility-mapping `internal_items` loop.
pub fn assert_each_contains(pairs: &[(&str, &str)]) {
    for &(path, needle) in pairs {
        assert!(
            non_test(&read_src(path)).contains(needle),
            "{path} must contain {needle:?}"
        );
    }
}
