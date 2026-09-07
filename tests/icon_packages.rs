//! Plan 112 task 4: the two first-party Phosphor icon packages must parse as
//! inert bounded geometry, ship identical semantic-key sets, stay in sync with
//! the generated host fallback, and regenerate without drift.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::process::Command;

use clay::packages::record::assemble_package_record;
use serde_json::Value;

const REGULAR_MANIFEST: &str = "packages/icons-phosphor-regular/package.json";
const DUOTONE_MANIFEST: &str = "packages/icons-phosphor-duotone/package.json";
const FALLBACK: &str = "frontend/src/icons/fallback.generated.ts";

fn read(path: &str) -> Value {
    let bytes = std::fs::read(path).unwrap_or_else(|error| panic!("read {path}: {error}"));
    serde_json::from_slice(&bytes).unwrap_or_else(|error| panic!("parse {path}: {error}"))
}

fn icon_pack(path: &str) -> serde_json::Map<String, Value> {
    read(path)["clay"]["contributions"]["iconPack"]
        .as_object()
        .unwrap_or_else(|| panic!("{path} must declare clay.contributions.iconPack"))
        .clone()
}

/// key -> icon entry (minus the key itself), sorted for stable comparison.
fn entries_by_key(path: &str) -> BTreeMap<String, Value> {
    let mut map = BTreeMap::new();
    for icon in icon_pack(path)["icons"].as_array().expect("icons array") {
        let mut entry = icon.as_object().expect("object").clone();
        let key = entry
            .remove("key")
            .expect("key field")
            .as_str()
            .expect("string key")
            .to_string();
        map.insert(key, Value::Object(entry));
    }
    map
}

#[test]
fn icon_packs_assemble_with_identical_semantic_key_sets() {
    for path in [REGULAR_MANIFEST, DUOTONE_MANIFEST] {
        let manifest = read(path);
        let record = assemble_package_record(&manifest)
            .unwrap_or_else(|error| panic!("{path} assembles: {}", error.message));
        let pack = record
            .contributions
            .icon_pack
            .as_ref()
            .unwrap_or_else(|| panic!("{path} must parse an iconPack contribution"));
        assert_eq!(
            pack.icons.len(),
            clay::shell::icons::CORE_ICON_KEYS.len(),
            "{path} must ship the full core semantic set"
        );
    }

    let regular: BTreeSet<String> = entries_by_key(REGULAR_MANIFEST).into_keys().collect();
    let duotone: BTreeSet<String> = entries_by_key(DUOTONE_MANIFEST).into_keys().collect();
    assert_eq!(regular, duotone, "packs must ship identical semantic keys");
    let core: BTreeSet<String> = clay::shell::icons::CORE_ICON_KEYS
        .iter()
        .map(ToString::to_string)
        .collect();
    assert_eq!(regular, core, "pack keys must be exactly the core set");
}

#[test]
fn icon_packs_are_bundled_trust_inventory_members() {
    // The checked-in inventory is the trust root; in-crate tests already
    // assert BUNDLED_PACKAGES matches it exactly.
    let inventory = std::fs::read_to_string("src/packages/bundled-inventory.toml")
        .expect("bundled-inventory.toml readable");
    for root in ["icons-phosphor-regular", "icons-phosphor-duotone"] {
        assert!(
            inventory.contains(&format!("root = \"{root}\"")),
            "{root} must be a bundled inventory root"
        );
        let name = read(&format!("packages/{root}/package.json"))["name"]
            .as_str()
            .expect("string name")
            .to_string();
        assert!(
            name.starts_with("@clay/"),
            "icon packs are first-party: {name}"
        );
    }
}

#[test]
fn host_fallback_matches_regular_pack_geometry() {
    // The fallback is generated from the Regular pack source; it must never
    // drift into a second hand-maintained icon map.
    let fallback = std::fs::read_to_string(FALLBACK)
        .unwrap_or_else(|error| panic!("read {FALLBACK}: {error}"));
    let payload = fallback
        .split("= ")
        .last()
        .expect("fallback payload")
        .trim_end()
        .trim_end_matches(';');
    let fallback_icons: BTreeMap<String, Value> = serde_json::from_str(payload)
        .unwrap_or_else(|error| panic!("fallback payload parses: {error}"));
    let manifest_icons = entries_by_key(REGULAR_MANIFEST);

    assert_eq!(
        fallback_icons.len(),
        manifest_icons.len(),
        "fallback and Regular pack must cover the same key count"
    );
    for (key, geometry) in &manifest_icons {
        let fallback_geometry = fallback_icons
            .get(key)
            .unwrap_or_else(|| panic!("fallback missing core key {key}"));
        assert_eq!(
            fallback_geometry, geometry,
            "fallback geometry drifted for {key}"
        );
    }
}

#[test]
fn icon_pack_generation_is_deterministic_and_drift_free() {
    // Regeneration must produce no diff (task 4 acceptance: generated
    // artifacts are deterministic and checked for drift).
    let output = Command::new("node")
        .arg("scripts/generate-icon-packs.mjs")
        .arg("--check")
        .output()
        .expect("run node (frontend toolchain requires node; CI provides it)");
    assert!(
        output.status.success(),
        "generation drift detected:\n{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn icon_packs_carry_no_executable_authority_or_extra_permissions() {
    for path in [REGULAR_MANIFEST, DUOTONE_MANIFEST] {
        let manifest = read(path);
        let clay = &manifest["clay"];
        assert_eq!(
            clay["permissions"].as_array().map(Vec::len),
            Some(0),
            "{path}: icon packs are inert data and declare no permissions"
        );
        assert_eq!(
            clay["modes"].as_array().map(Vec::len),
            Some(0),
            "{path}: icon packs declare no modes"
        );
        let pack_dir = std::path::Path::new(path)
            .parent()
            .expect("parent directory");
        let load_entry = pack_dir.join(clay["loadEntry"].as_str().expect("loadEntry path"));
        let source = std::fs::read_to_string(&load_entry)
            .unwrap_or_else(|error| panic!("read {}: {error}", load_entry.display()));
        assert!(
            source.contains("export {}"),
            "{} load entry must be the generated no-op, not runtime registration",
            load_entry.display()
        );
        assert!(
            pack_dir.join("LICENSE").is_file(),
            "{path}: MIT license notice must ship with the pack"
        );
        assert!(
            pack_dir.join("docs/index.md").is_file(),
            "{path}: package docs must ship with the pack"
        );
    }
}
