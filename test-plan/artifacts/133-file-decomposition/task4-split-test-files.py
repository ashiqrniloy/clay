#!/usr/bin/env python3
"""Plan 133 task 4: split the four giant inline test modules into focused suites.

Each `src/**/tests.rs` becomes `src/**/tests/mod.rs` (module doc + all imports +
shared helpers + `mod <suite>;` declarations) plus one file per contiguous
thematic run of tests, e.g. `src/server/js_runtime/tests/runtime_third_party.rs`.

Tests are moved verbatim. The only textual edit is the explicit `super::` chain
inside moved test bodies: those items now live one module deeper
(`tests::<suite>` instead of `tests`), so `super::X` becomes `super::super::X`
(maximal chains are extended by one level). Bare-name references keep working
through each suite's `use super::*;`, which is why the shared helper items stay
in `tests/mod.rs`.

Run from the repository root. `--dry` validates and prints the map only.
"""
import re
import sys

DECL = re.compile(
    r'^(?:pub(?:\([a-z]+\))? )?(?:async )?(?:fn|struct|enum|impl|mod|const|static|type|trait|union)\b'
)
FN_NAME = re.compile(r'^(?:pub(?:\([a-z]+\))? )?(?:async )?fn ([A-Za-z_][A-Za-z0-9_]*)')

# (source file, module title, [(suite file stem, first test fn name), ...])
LADDERS = [
    (
        'src/server/js_runtime/tests.rs',
        'Stay-in-crate unit tests for the Clay JS runtime service: worker lifecycle,\n//! lane isolation, configuration/init-js loading, package + module-loader\n//! confinement, language/completion/parse facades, markdown adapters, themes,\n//! icon packs, and agent host wiring.',
        [
            ('runtime_third_party', 'js_runtime_evaluates_controlled_module'),
            ('language_facades', 'js_parse_handler_bridge_runs_registered_markdown_handler'),
            ('language_packages', 'load_package_registers_first_party_syntax_grammars'),
            ('runtime_limits', 'js_parse_handler_timeout_uses_registered_budget'),
            ('configuration_runtime', 'configuration_runtime_loads_init_js_fixture'),
            ('config_fixture_workflows', 'smoke_config_fixture_publishes_runtime_sdui_snapshot'),
            ('document_and_git_facades', 'document_facade_open_status_list_round_trip'),
            ('configuration_keybindings', 'configuration_runtime_rejects_traversal_and_urls'),
            ('facades_and_modes', 'runtime_imports_modes_commands_and_packages_facades'),
            ('editor_control', 'editor_control_gate_enforces_permission_and_declared_mode'),
            (
                'language_registration_and_primitives',
                'language_commands_are_package_prefixed_and_server_first_with_provenance',
            ),
            ('decoration_and_diagnostics_facades', 'phase18_parse_and_decoration_facades_are_runtime_backed'),
            ('markdown_windowed_adapter', 'markdown_parser_adapter_publishes_viewport_bounded_decorations'),
            ('package_adoption', 'third_party_config_load_fails_with_pending_adoption_diagnostic'),
            ('runtime_errors_and_authorization', 'ordinary_typing_does_not_enter_js_runtime'),
            ('load_package_and_module_loader', 'load_package_user_installed_default_loads_from_init_js'),
            ('themes_and_appearance', 'set_theme_resolves_first_party_gruvbox_theme'),
            ('coding_agent_and_launcher', 'first_party_example_loads_coding_agent_with_one_uncommented_line'),
            ('runtime_themes_and_typography', 'set_theme_resolves_first_party_modus_themes'),
            ('load_package_markdown_defaults', 'load_package_resolves_and_activates_first_party_markdown_end_to_end'),
            ('editor_layout_and_design_system', 'clay_module_loader_resolves_trusted_helper_exports'),
            ('agent_lane_host_wiring', 'agent_facade_fails_closed_without_an_attached_host'),
            ('plan112_icon_packs', 'plan112_load_then_select_recommended_path_activates_pack'),
            ('lanes_and_queues', 'revoked_package_commands_refused_per_lane'),
        ],
    ),
    (
        'src/server/connection/tests.rs',
        'Stay-in-crate unit tests for a single server connection: handshake/reclaim,\n//! path-browser authority, menus and the control centre, tab lifecycle, edit\n//! ack/resync, viewport rendering, extracted handlers, and runtime diagnostics.',
        [
            ('language_intelligence', 'language_intelligence_window_uses_active_behavior_mode'),
            ('settings_and_sdui_actions', 'sdui_actions_and_keybinding_intents_share_command_execution_path'),
            ('handshake_and_reclaim', 'server_accepts_hello_and_sends_snapshot'),
            ('workspace_authority_and_path_browser', 'cross_tab_workspace_and_document_authority_is_fail_closed'),
            ('path_browser_grants_and_reload', 'path_browser_workspace_open_rejects_vanished_directory'),
            ('control_center_and_menus', 'menu_intents_for_unknown_sessions_produce_bounded_diagnostics'),
            ('tab_lifecycle', 'tab_switch_cancels_the_active_server_menu_session'),
            ('connection_identity', 'forged_client_identity_is_rejected_for_every_message_family'),
            ('protocol_and_bootstrap', 'save_reload_status_list_enforce_connection_owned_access'),
            ('edit_ack_and_resync', 'client_receives_js_generated_sdui_snapshot'),
            ('document_open_and_viewport', 'connection_open_document_sends_snapshot_and_manifest_without_full_document_on_edit_ack'),
            ('markdown_modes', 'default_init_js_load_package_powers_selected_markdown_open'),
            ('document_rendering_and_lifecycle', 'open_document_renders_before_background_parse_completes'),
            ('runtime_diagnostics_and_extracted_handlers', 'runtime_diagnostic_store_deduplicates_and_bounds'),
        ],
    ),
    (
        'src/client/tests.rs',
        'Stay-in-crate unit tests for the IPC client: edit queue and acks, events,\n//! viewport/framing, and real-server end-to-end flows over Unix sockets and\n//! Windows named pipes.',
        [
            ('edit_queue_and_resync', 'invalid_behavior_version_rejection_requests_resync'),
            ('viewport_and_framing', 'bounded_edit_queue_applies_backpressure'),
            ('intents_and_acks', 'sdui_button_action_emits_server_intent'),
            ('end_to_end_handshake', 'client_installs_minimal_behavior_manifest'),
            ('client_events', 'client_forwards_document_opened_without_replacing_live_sync_state'),
            ('real_server_end_to_end', 'end_to_end_second_client_gets_independent_welcome_document'),
            ('windows_named_pipe', 'windows_named_pipe_client_receives_initial_snapshot'),
            ('real_server_tabs', 'real_server_tab_command_new_registers_and_rejected_activate_pushes_reconcile'),
        ],
    ),
    (
        'src/server/workspace/tests.rs',
        'Stay-in-crate unit tests for workspace state: document open/dedup, path\n//! authorization, dirty state and saves, atomic-save hardening, per-client\n//! document budgets, workspace roots, user browse, and directory listing.',
        [
            ('document_open_dedup', 'duplicate_open_reuses_document_and_preserves_lease_policy'),
            ('path_authorization', 'workspace_rejects_path_traversal_outside_root'),
            ('dirty_state_and_save', 'file_backed_document_dirty_state_tracks_accepted_edits_and_clean_marking'),
            ('save_concurrency', 'concurrent_save_different_documents'),
            ('atomic_save_hardening', 'atomic_save_ignores_precreated_temp_file'),
            ('document_lifecycle', 'open_documents_enforce_per_client_ceiling'),
            ('workspace_roots', 'bounded_ignore_grammar_supports_root_paths_and_rejects_unsupported_rules'),
            ('user_browse', 'list_directory_returns_immediate_children'),
            ('directory_listing', 'list_directory_respects_max_depth'),
        ],
    ),
]


# suite file stem -> cfg attribute for suites whose tests are platform-gated
# end-to-end (`#[cfg(windows)]`), so the module file matches the original gating
SUITE_CFG = {
    ('src/client/tests.rs', 'windows_named_pipe'): '#[cfg(windows)]',
}


def items(lines):
    """Top-level items with their contiguous attribute/doc prefix (0-based)."""
    blocks = []
    pending = None
    depth = 0
    for i, line in enumerate(lines):
        s = line.strip()
        if depth > 0:
            depth += s.count('[') - s.count(']')
            continue
        if s.startswith('#['):
            if pending is None:
                pending = i
            depth += s.count('[') - s.count(']')
            continue
        if s.startswith('///') or s.startswith('//!') or s.startswith('//'):
            if pending is None:
                pending = i
            continue
        if not s:
            pending = None
            continue
        if DECL.match(line):
            start = pending if pending is not None else i
            name = None
            m = FN_NAME.match(s)
            if m:
                name = m.group(1)
            blocks.append({'name': name, 'start': start, 'decl': i,
                           'attrs': ' '.join(x.strip() for x in lines[start:i]),
                           'text': s})
        pending = None
    for k, b in enumerate(blocks):
        end = blocks[k + 1]['start'] if k + 1 < len(blocks) else len(lines)
        while end > b['decl'] and not lines[end - 1].strip():
            end -= 1
        b['end'] = end
    return blocks


def code_of(line):
    """Line with line comments and string literals removed (reference scan)."""
    line = re.sub(r'"(?:[^"\\]|\\.)*"', '""', line)
    line = line.split('//')[0]
    return line


INCLUDE = re.compile(r'(include(?:_str|_bytes)?!\(\s*")([^"$][^"]*)(")')


def bump_super(line):
    """Items moved one module deeper: `super::` and `include_*!` paths gain a level."""
    line = re.sub(r'super::(?:super::)*', lambda m: 'super::' + m.group(0), line)

    def fix(m):
        arg = m.group(2)
        return m.group(1) + (arg if arg.startswith('/') else '../' + arg) + m.group(3)

    return INCLUDE.sub(fix, line)


def split(path, title, ladder, dry):
    lines = open(path).read().split('\n')
    blocks = items(lines)
    tests = [b for b in blocks if '#[test]' in b['attrs'] or '#[tokio::test]' in b['attrs']]
    for b in blocks:
        b['test'] = b in tests

    by_name = {b['name']: b for b in tests}
    missing = [n for _, n in ladder if n not in by_name]
    assert not missing, f'{path}: ladder names not found: {missing}'
    starts = [by_name[n]['start'] for _, n in ladder]
    assert starts == sorted(starts) and len(set(starts)) == len(starts), f'{path}: ladder out of order'

    # every test lands in exactly one suite; check cross-suite references
    for i, b in enumerate(tests):
        nxt = starts + [10 ** 9]
        suite = [name for (name, _), s in zip(ladder, starts) if b['start'] >= s][-1]
        b['suite'] = suite

    parts = {name: [] for name, _ in ladder}
    for b in tests:
        parts[b['suite']].append(b)

    # a test referenced by a test in another suite would lose reachability
    test_names = {b['name'] for b in tests}
    for b in tests:
        for i in range(b['start'], b['end']):
            code = code_of(lines[i])
            if not code:
                continue
            for name in test_names:
                if name == b['name'] or not re.search(r'(?<![\w])' + name + r'(?![\w])', code):
                    continue
                if re.match(r'\s*(?:async )?fn ' + name + r'\b', code):
                    continue
                other = by_name[name]['suite']
                assert other == b['suite'], (
                    f'{path}:{i + 1}: {b["name"]} ({b["suite"]}) references '
                    f'{name} ({other})'
                )

    # assemble
    root_lines = []
    removed = 0
    cursor = 0
    for b in blocks:
        if not b['test']:
            continue
        root_lines += lines[cursor:b['start']]
        removed += b['end'] - b['start']
        cursor = b['end']
    root_lines += lines[cursor:]
    root = '\n'.join(root_lines)

    first_test = min(b['start'] for b in tests)
    decls = '\n'.join(
        (SUITE_CFG.get((path, name), '') + '\n' if SUITE_CFG.get((path, name)) else '')
        + f'mod {name};'
        for name, _ in ladder
    )
    # place the suite map right before the first test item was removed: after the
    # header block (imports + helpers that precede the first test).
    root_items = [b for b in blocks if not b['test']]
    anchor = max((b['end'] for b in root_items if b['end'] <= first_test), default=0)
    head = '\n'.join(lines[0:anchor]) if anchor else ''
    tail = root[len(head):]
    root = head.rstrip('\n') + '\n\n' + f'// Test suites (see each file for its scope).\n{decls}\n' + tail.lstrip('\n')

    out = {}
    out[path.replace('tests.rs', 'tests/mod.rs')] = f'//! {title}\n' + root
    bumped = 0
    for name, _ in ladder:
        body = []
        for b in parts[name]:
            body.append('\n'.join(bump_super(l) for l in lines[b['start']:b['end']]))
            bumped += sum(
                1 for l in lines[b['start']:b['end']]
                if re.search(r'(?<![\w:])super::', l) or INCLUDE.search(l)
            )
        text = '\n\n'.join(body)
        out[path.replace('tests.rs', f'tests/{name}.rs')] = f'use super::*;\n\n{text}\n'
    return out, dict(relocated=path, tests=len(tests), suites=len(ladder), bumped=bumped,
                     removed=removed, lines=len(lines))


def main():
    dry = '--dry' in sys.argv
    for path, title, ladder in LADDERS:
        parts = {name: [] for name, _ in ladder}
        lines = open(path).read().split('\n')
        blocks = items(lines)
        tests = [b for b in blocks if '#[test]' in b['attrs'] or '#[tokio::test]' in b['attrs']]
        starts = {
            name: next(b['start'] for b in tests if b['name'] == first_test)
            for name, first_test in ladder
        }
        for b in tests:
            parts[[n for n, _ in ladder if b['start'] >= starts[n]][-1]].append(b)
        out, stat = split(path, title, ladder, dry)
        print(f"{path}: {stat['tests']} tests -> {stat['suites']} suites, "
              f"{stat['bumped']} super:: lines bumped, {stat['removed']} lines removed from root")
        for name, _ in ladder:
            bs = parts[name]
            size = sum(b['end'] - b['start'] for b in bs)
            print(f"    {name}: {len(bs)} tests, {size} lines")
        if dry:
            continue
        import os
        for rel, text in out.items():
            os.makedirs(os.path.dirname(rel), exist_ok=True)
            with open(rel, 'w') as fh:
                fh.write(text)
        os.remove(path)


if __name__ == '__main__':
    main()