use std::path::Path;

use clay::perf::fixtures::{
    FixtureError, FixtureKind, FixtureSpec, generate_fixture, generate_fixture_file,
    validate_output_path,
};

#[test]
fn perf_fixture_generation_is_deterministic() {
    let spec = FixtureSpec {
        kind: FixtureKind::MixedUnicode,
        size_bytes: 16 * 1024,
        seed: 42,
    };
    let mut first = Vec::new();
    let mut second = Vec::new();

    generate_fixture(&spec, &mut first).expect("first fixture generates");
    generate_fixture(&spec, &mut second).expect("second fixture generates");

    assert_eq!(first, second);
    assert_eq!(first.len(), spec.size_bytes);
    std::str::from_utf8(&first).expect("fixture is valid UTF-8");
}

#[test]
fn perf_fixture_generation_rejects_unsafe_output_paths() {
    for path in [
        Path::new("target/perf-fixtures/../secret.txt"),
        Path::new("docs/perf.txt"),
        Path::new("Cargo.toml"),
    ] {
        let error = validate_output_path(path).expect_err("unsafe path should be rejected");
        assert!(matches!(error, FixtureError::UnsafeOutputPath { .. }));
    }
}

#[test]
fn perf_fixture_shapes_include_unicode_and_long_lines() {
    let unicode = bytes_for(FixtureKind::MixedUnicode, 8192);
    let unicode_text = std::str::from_utf8(&unicode).expect("unicode fixture is UTF-8");
    assert!(
        unicode_text.contains('🦀') || unicode_text.contains('中') || unicode_text.contains('é')
    );

    let long_lines = bytes_for(FixtureKind::LongLines, 20 * 1024);
    let long_text = std::str::from_utf8(&long_lines).expect("long-line fixture is UTF-8");
    assert!(
        long_text.lines().any(|line| line.len() >= 4096),
        "long-line fixture should contain viewport/layout stress lines"
    );

    let newline_heavy = bytes_for(FixtureKind::NewlineHeavy, 4096);
    let newline_text = std::str::from_utf8(&newline_heavy).expect("newline fixture is UTF-8");
    assert!(newline_text.contains("\n\n\n"));
}

#[test]
fn perf_fixture_file_writes_only_under_allowed_fixture_roots() {
    let output = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("perf-fixtures")
        .join("test-small.txt");
    let spec = FixtureSpec::new(FixtureKind::ManyShortLines, 1024);

    let written = generate_fixture_file(&spec, &output).expect("allowed target fixture writes");

    assert_eq!(written, output);
    let bytes = std::fs::read(&written).expect("generated fixture can be read");
    assert_eq!(bytes.len(), spec.size_bytes);
    let _ = std::fs::remove_file(written);
}

fn bytes_for(kind: FixtureKind, size_bytes: usize) -> Vec<u8> {
    let spec = FixtureSpec {
        kind,
        size_bytes,
        seed: 7,
    };
    let mut bytes = Vec::new();
    generate_fixture(&spec, &mut bytes).expect("fixture generates");
    bytes
}
