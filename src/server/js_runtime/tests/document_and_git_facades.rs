use super::*;

#[tokio::test]
async fn document_facade_open_status_list_round_trip() {
    let config_root = config_fixture("document-facade");
    let workspace_root = config_root.join("workspace");
    fs::create_dir(&workspace_root).unwrap();
    fs::write(workspace_root.join("note.txt"), "hello").unwrap();
    fs::write(
        config_root.join("init.js"),
        r#"
        import {
          serverGetDocumentStatus,
          serverListDocuments,
          serverOpenDocument,
          serverReloadDocument,
          serverSaveDocument,
        } from "clay:documents";

        const opened = await serverOpenDocument({ workspaceRootId: "1", path: "note.txt" });
        const status = await serverGetDocumentStatus(opened.metadata.documentId);
        const saved = await serverSaveDocument({ documentId: opened.metadata.documentId });
        const reloaded = await serverReloadDocument({ documentId: opened.metadata.documentId });
        const documents = await serverListDocuments();
        Deno.core.ops.op_clay_runtime_record(`${opened.text}:${status.path}:${saved.dirty}:${reloaded.text}:${documents.length}`);
        "#,
    )
    .unwrap();
    let mut workspace = WorkspaceState::new();
    workspace.add_root(&workspace_root).unwrap();

    let result = ClayJsRuntimeService::default()
        .load_configuration_from_root_with_workspace(config_root, Arc::new(Mutex::new(workspace)))
        .await
        .unwrap();

    assert_eq!(result.op_records, vec!["hello:note.txt:false:hello:1"]);
}

#[tokio::test]
async fn documents_open_over_budget_returns_typed_error() {
    use crate::perf::budgets::DOCUMENTS_OP_MAX_DOCUMENT_BYTES;

    let config_root = config_fixture("document-open-budget");
    let workspace_root = config_root.join("workspace");
    fs::create_dir(&workspace_root).unwrap();
    fs::write(workspace_root.join("note.txt"), "hello").unwrap();

    let mut workspace = WorkspaceState::new();
    let workspace_root_id = workspace.add_root(&workspace_root).unwrap();
    let workspace = Arc::new(Mutex::new(workspace));
    // The runtime already holds the document (an earlier op call, when the file
    // was small). The client/editor open path is deliberately not gated here, so
    // the reload leg exercises the bypass this budget closes: grow the file on
    // disk, then reload through the op.
    let preopened = crate::server::workspace::open_existing_file_unlocked(
        &workspace,
        workspace_root_id,
        workspace_root.join("note.txt"),
        0,
    )
    .await
    .unwrap();

    // One byte over budget, filled with a marker so the assertions prove the
    // payload never reached the JavaScript error path.
    let payload = "SECRETPAYLOAD".repeat(DOCUMENTS_OP_MAX_DOCUMENT_BYTES / 13 + 1);
    assert!(payload.len() > DOCUMENTS_OP_MAX_DOCUMENT_BYTES);
    fs::write(workspace_root.join("note.txt"), &payload).unwrap();
    fs::write(workspace_root.join("large.txt"), &payload).unwrap();

    fs::write(
        config_root.join("init.js"),
        format!(
            r#"
        import {{ serverOpenDocument, serverReloadDocument }} from "clay:documents";

        const record = async (label, run) => {{
          try {{
            await run();
            Deno.core.ops.op_clay_runtime_record(`${{label}}:accepted`);
          }} catch (error) {{
            const message = String(error);
            Deno.core.ops.op_clay_runtime_record(
              `${{label}}:` +
              `${{message.includes("documents.document_too_large") ? "typed" : message}}:` +
              `${{message.includes("SECRETPAYLOAD") ? "leaked-text" : "no-text"}}`
            );
          }}
        }};

        await record("open", () =>
          serverOpenDocument({{ workspaceRootId: "1", path: "large.txt" }})
        );
        await record("reload", () =>
          serverReloadDocument({{ documentId: "{document_id}" }})
        );
        "#,
            document_id = preopened.document_id
        ),
    )
    .unwrap();

    let result = ClayJsRuntimeService::default()
        .load_configuration_from_root_with_workspace(config_root, Arc::clone(&workspace))
        .await
        .unwrap();

    assert_eq!(
        result.op_records,
        vec!["open:typed:no-text", "reload:typed:no-text"]
    );
}

#[tokio::test]
async fn documents_open_under_budget_unchanged() {
    let config_root = config_fixture("document-open-budget-under");
    let workspace_root = config_root.join("workspace");
    fs::create_dir(&workspace_root).unwrap();
    fs::write(workspace_root.join("note.txt"), "hello").unwrap();
    fs::write(
        config_root.join("init.js"),
        r#"
        import { serverOpenDocument, serverReloadDocument } from "clay:documents";

        const opened = await serverOpenDocument({ workspaceRootId: "1", path: "note.txt" });
        const reloaded = await serverReloadDocument({ documentId: opened.metadata.documentId });
        const metadata = opened.metadata;
        Deno.core.ops.op_clay_runtime_record(
          `${Object.keys(opened).sort().join(",")}|` +
          `${Object.keys(metadata).sort().join(",")}|` +
          `${metadata.documentId}:${metadata.version}:${metadata.readOnly}:${metadata.leaseId}:` +
          `${metadata.dirty}:${metadata.workspaceRootId}:${metadata.path}|` +
          `${opened.text}:${reloaded.text}:${reloaded.metadata.path}`
        );
        "#,
    )
    .unwrap();
    let mut workspace = WorkspaceState::new();
    workspace.add_root(&workspace_root).unwrap();

    let result = ClayJsRuntimeService::default()
        .load_configuration_from_root_with_workspace(config_root, Arc::new(Mutex::new(workspace)))
        .await
        .unwrap();

    // Golden result contract for the under-budget path: same JSON shape, same
    // metadata values, same text as before the budget existed.
    assert_eq!(
        result.op_records,
        vec![
            "metadata,text|dirty,documentId,leaseId,path,readOnly,version,workspaceRootId|"
                .to_string()
                + "1:1:false:1:false:1:note.txt|hello:hello:note.txt"
        ]
    );
}

#[tokio::test]
async fn document_facade_save_rejects_future_known_version() {
    let config_root = config_fixture("document-save-version");
    let workspace_root = config_root.join("workspace");
    fs::create_dir(&workspace_root).unwrap();
    fs::write(workspace_root.join("note.txt"), "hello").unwrap();
    fs::write(
        config_root.join("init.js"),
        r#"
        import { serverOpenDocument, serverSaveDocument } from "clay:documents";
        const opened = await serverOpenDocument({ workspaceRootId: "1", path: "note.txt" });
        try {
          await serverSaveDocument({
            documentId: opened.metadata.documentId,
            knownVersion: opened.metadata.version + 1,
          });
          Deno.core.ops.op_clay_runtime_record("accepted");
        } catch (error) {
          Deno.core.ops.op_clay_runtime_record(String(error).includes("claims version") ? "rejected" : String(error));
        }
        "#,
    )
    .unwrap();
    let mut workspace = WorkspaceState::new();
    workspace.add_root(&workspace_root).unwrap();

    let result = ClayJsRuntimeService::default()
        .load_configuration_from_root_with_workspace(config_root, Arc::new(Mutex::new(workspace)))
        .await
        .unwrap();

    assert_eq!(result.op_records, vec!["rejected"]);
}

#[tokio::test]
async fn workspace_roots_facade_reports_authorized_roots() {
    let config_root = config_fixture("workspace-facade");
    let workspace_root = config_root.join("project");
    fs::create_dir(&workspace_root).unwrap();
    fs::write(
        config_root.join("init.js"),
        r#"
        import { serverListWorkspaceRoots } from "clay:workspace";
        const roots = await serverListWorkspaceRoots();
        Deno.core.ops.op_clay_runtime_record(`${roots.length}:${roots[0].workspaceRootId}:${roots[0].displayName}`);
        "#,
    )
    .unwrap();
    let mut workspace = WorkspaceState::new();
    workspace.add_root(&workspace_root).unwrap();

    let result = ClayJsRuntimeService::default()
        .load_configuration_from_root_with_workspace(config_root, Arc::new(Mutex::new(workspace)))
        .await
        .unwrap();

    assert_eq!(result.op_records, vec!["1:1:project"]);
}

#[tokio::test]
async fn git_facade_lists_refreshes_and_commands_statuses() {
    let config_root = config_fixture("git-facade");
    let repo_root = config_root.join("repo");
    let plain_root = config_root.join("plain");
    fs::create_dir(&repo_root).unwrap();
    fs::create_dir(&plain_root).unwrap();
    init_git_repo(&repo_root);
    fs::write(repo_root.join("tracked.txt"), "base").unwrap();
    git(&repo_root, ["add", "."]);
    git(&repo_root, ["commit", "-m", "initial"]);
    fs::write(repo_root.join("tracked.txt"), "changed").unwrap();
    fs::write(
        config_root.join("init.js"),
        r#"
        import { serverListGitStatuses, serverRefreshGitStatus } from "clay:git";
        import { serverExecuteCommand } from "clay:commands";
        const cold = await serverListGitStatuses();
        const repo = await serverRefreshGitStatus({ workspaceRootId: cold[0].workspaceRootId });
        const plain = await serverRefreshGitStatus({ workspaceRootId: cold[1].workspaceRootId });
        const listed = await serverExecuteCommand("git.listStatuses");
        const refreshed = await serverExecuteCommand("git.refreshStatus", { workspaceRootId: cold[0].workspaceRootId });
        Deno.core.ops.op_clay_runtime_record(`${cold.length}:${cold[0].refreshState.kind}:${repo.snapshot.head.kind}:${repo.snapshot.dirty}:${plain.snapshot.lastRefresh.kind}:${listed.status.kind}:${listed.status.statuses.length}:${refreshed.status.action}`);
        "#,
    )
    .unwrap();
    let mut workspace = WorkspaceState::new();
    workspace.add_root(&repo_root).unwrap();
    workspace.add_root(&plain_root).unwrap();

    let result = ClayJsRuntimeService::default()
        .load_configuration_from_root_with_workspace(config_root, Arc::new(Mutex::new(workspace)))
        .await
        .unwrap();

    assert_eq!(
        result.op_records,
        vec!["2:idle:branch:true:non-repository:git:2:refreshed"]
    );
}

#[tokio::test]
async fn git_package_loads_and_publishes_read_only_status() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let config_root = config_fixture("git-package-load");
    let repo_root = config_root.join("repo");
    let plain_root = config_root.join("plain");
    fs::create_dir(&repo_root).unwrap();
    fs::create_dir(&plain_root).unwrap();
    init_git_repo(&repo_root);
    fs::write(repo_root.join("tracked.txt"), "base").unwrap();
    git(&repo_root, ["add", "."]);
    git(&repo_root, ["commit", "-m", "initial"]);
    fs::write(repo_root.join("tracked.txt"), "changed").unwrap();
    fs::write(
        config_root.join("init.js"),
        r#"
        import { loadPackage } from "clay:packages";
        import { serverListGitStatuses, serverRefreshGitStatus } from "clay:git";

        // Warm the cache first so the package status panel renders branch state.
        const cold = await serverListGitStatuses();
        await serverRefreshGitStatus({ workspaceRootId: cold[0].workspaceRootId });
        // `loadPackage("@clay/git")` runs the load entry, which publishes a
        // read-only status tree from cached clay:git data. No throw => the
        // status data path works against a repo + plain root.
        const summary = await loadPackage("@clay/git");
        const warm = await serverListGitStatuses();
        Deno.core.ops.op_clay_runtime_record(`${summary.name}:${summary.apiPrefix}:${summary.permissions.length}:${summary.contributions.sdui}:${warm.length}:${warm[0].snapshot.head.kind}:${warm[0].snapshot.dirty}`);
        "#,
    )
    .unwrap();
    let mut workspace = WorkspaceState::new();
    workspace.add_root(&repo_root).unwrap();
    workspace.add_root(&plain_root).unwrap();

    let result = ClayJsRuntimeService::default()
        .load_configuration_from_root_with_workspace(config_root, Arc::new(Mutex::new(workspace)))
        .await
        .unwrap();

    assert_eq!(result.op_records, vec!["@clay/git:git:0:1:2:branch:true"]);
}

#[tokio::test]
async fn git_package_declares_no_mutation_or_network_authority() {
    // Phase 18.13: prove @clay/git is read-only. It declares no permissions
    // (no network/shell/filesystem/mutation), registers no package commands,
    // and exposes no configuration/package options (fixed safe defaults).
    // Mutating Git operations and config knobs are intentionally out of scope.
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let config_root = config_fixture("git-package-authority");
    fs::write(
        config_root.join("init.js"),
        r#"
        import { loadPackage } from "clay:packages";

        const summary = await loadPackage("@clay/git");
        const perms = summary.permissions.join(",");
        const mutating = ["filesystem", "network", "shell", "wasm", "ai-tools",
          "workspace-mutation", "native-ui", "client-runtime", "raw-ops",
          "package-control", "package-import"];
        const leaked = mutating.filter((m) => perms.includes(m)).join(",");
        Deno.core.ops.op_clay_runtime_record(
          `${perms.length}:${summary.contributions.commands}:` +
          `${summary.contributions.configuration}:${summary.contributions.packageOptions}:${leaked}`
        );
        "#,
    )
    .unwrap();
    let workspace = WorkspaceState::new();
    let result = ClayJsRuntimeService::default()
        .load_configuration_from_root_with_workspace(config_root, Arc::new(Mutex::new(workspace)))
        .await
        .unwrap();

    // perms:commands:configuration:packageOptions:leaked — all zero/empty
    assert_eq!(result.op_records, vec!["0:0:0:0:"]);
}

#[tokio::test]
async fn document_facade_rejects_unauthorized_paths() {
    let parent = config_fixture("document-facade-reject");
    let config_root = parent.join("config");
    let workspace_root = parent.join("workspace");
    fs::create_dir(&config_root).unwrap();
    fs::create_dir(&workspace_root).unwrap();
    fs::write(parent.join("outside.txt"), "secret").unwrap();
    fs::write(
        config_root.join("init.js"),
        r#"
        import { serverOpenDocument } from "clay:documents";
        await serverOpenDocument({ workspaceRootId: "1", path: "../outside.txt" });
        "#,
    )
    .unwrap();
    let mut workspace = WorkspaceState::new();
    workspace.add_root(&workspace_root).unwrap();

    let error = ClayJsRuntimeService::default()
        .load_configuration_from_root_with_workspace(config_root, Arc::new(Mutex::new(workspace)))
        .await
        .unwrap_err();

    assert!(matches!(error, ClayRuntimeError::Runtime(_)));
    assert!(error.to_string().contains("documents.open_failed"));
    assert!(error.to_string().contains("outside the authorized root"));
}
