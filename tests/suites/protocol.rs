#[path = "../clay_js_api_inventory.rs"]
mod clay_js_api_inventory;
#[path = "../clay_js_doc_registry.rs"]
mod clay_js_doc_registry;
#[path = "../clay_js_facade_layout.rs"]
mod clay_js_facade_layout;
#[path = "../manual_smoke_docs.rs"]
mod manual_smoke_docs;
#[path = "../package_loading_docs.rs"]
mod package_loading_docs;
#[path = "../perf_fixtures.rs"]
mod perf_fixtures;
#[path = "../performance_budgets.rs"]
mod performance_budgets;
#[path = "../performance_protocol.rs"]
mod performance_protocol;
#[path = "../primitives_docs.rs"]
mod primitives_docs;

#[test]
fn integration_suite_inventory_assigns_every_source_once() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut sources: Vec<_> = std::fs::read_dir(root.join("tests"))
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
        .filter_map(|entry| {
            entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "rs")
                .then(|| entry.file_name().to_string_lossy().into_owned())
        })
        .collect();
    let suites = ["editor", "protocol", "runtime", "security"];
    let mut assigned = Vec::new();
    for suite in suites {
        let source =
            std::fs::read_to_string(root.join("tests/suites").join(format!("{suite}.rs"))).unwrap();
        assigned.extend(source.lines().filter_map(|line| {
            line.strip_prefix("#[path = \"../")
                .and_then(|line| line.strip_suffix("\"]"))
                .map(str::to_owned)
        }));
    }
    sources.sort();
    assigned.sort();
    assert_eq!(assigned, sources);
}
