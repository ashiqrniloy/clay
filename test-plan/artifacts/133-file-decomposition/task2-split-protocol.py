#!/usr/bin/env python3
"""Split src/protocol/mod.rs into family modules (plan 133 task 2)."""
import re
import sys

SRC = 'src/protocol/mod.rs'
DRY = '--dry' in sys.argv

NEW_FILES = {
    'behavior.rs': (
        "//! Clay behavior-manifest family: manifests, key bindings, command\n"
        "//! declarations, and routing/lock policy.\n"
    ),
    'editor_rules.rs': (
        "//! Editor behavior/layout rule shapes carried by [`BehaviorManifest`].\n"
    ),
    'caret.rs': (
        "//! Client caret-appearance override contract (`setCursorStyle`).\n"
    ),
    'typography.rs': (
        "//! Typography contract: font roles, profiles, ligatures, and the active\n"
        "//! typography snapshot.\n"
    ),
    'document.rs': (
        "//! Document metadata/access and the bounded document-head/chunk contract.\n"
    ),
    'launcher.rs': (
        "//! Launcher setup snapshots (workspace roots, agent entries).\n"
    ),
    'theme.rs': (
        "//! Active theme snapshot and wire design-token overrides.\n"
    ),
    'shell.rs': (
        "//! Shell-owned client state: preferences and the server-authoritative tab\n"
        "//! registry.\n"
    ),
    'messages.rs': (
        "//! Client/server wire envelopes and their rejection payloads.\n"
    ),
}

ASSIGN = {
    # behavior.rs
    'BehaviorManifest': 'behavior.rs',
    'default_keymaps': 'behavior.rs',
    'ctrl_key': 'behavior.rs',
    'ctrl_shift_key': 'behavior.rs',
    'ctrl_alt_key': 'behavior.rs',
    'ctrl_alt_shift_key': 'behavior.rs',
    'default_commands': 'behavior.rs',
    'BehaviorScope': 'behavior.rs',
    'KeyBindingRule': 'behavior.rs',
    'KeyStroke': 'behavior.rs',
    'KeyCode': 'behavior.rs',
    'KeyModifiers': 'behavior.rs',
    'KeyBindingContext': 'behavior.rs',
    'CommandDeclaration': 'behavior.rs',
    'CommandAuthority': 'behavior.rs',
    'RoutingPolicy': 'behavior.rs',
    'LockScope': 'behavior.rs',
    'tests': 'behavior.rs',
    # editor_rules.rs
    'EditOperation': 'editor_rules.rs',
    'EditorIntent': 'editor_rules.rs',
    'WordSeparatorPolicy': 'editor_rules.rs',
    'ParagraphStyle': 'editor_rules.rs',
    'LineMovementStyle': 'editor_rules.rs',
    'MovementRules': 'editor_rules.rs',
    'EditorBehaviorRules': 'editor_rules.rs',
    'EditorLayoutRules': 'editor_rules.rs',
    'WrapPolicy': 'editor_rules.rs',
    'EditorChrome': 'editor_rules.rs',
    'TextEditCapability': 'editor_rules.rs',
    'EnterRule': 'editor_rules.rs',
    'TabRule': 'editor_rules.rs',
    'TabMode': 'editor_rules.rs',
    'PairRule': 'editor_rules.rs',
    'PairRuleContext': 'editor_rules.rs',
    'CommentContinuationRule': 'editor_rules.rs',
    'ElectricEffect': 'editor_rules.rs',
    'ElectricCharacterRule': 'editor_rules.rs',
    'AutocompleteTrigger': 'editor_rules.rs',
    # caret.rs
    'CaretShape': 'caret.rs',
    'BlinkStyle': 'caret.rs',
    'CaretStyle': 'caret.rs',
    'MAX_CARET_WIDTH_PX': 'caret.rs',
    'MAX_CARET_HEIGHT_PCT': 'caret.rs',
    'MAX_CARET_BLINK_PHASE_MS': 'caret.rs',
    'MAX_CARET_SMOOTH_ANIMATION_MS': 'caret.rs',
    'CaretStyleValidationError': 'caret.rs',
    # typography.rs
    'FontRole': 'typography.rs',
    'DocumentFontRole': 'typography.rs',
    'TextThemeOverride': 'typography.rs',
    'MAX_FONT_FAMILIES_PER_PROFILE': 'typography.rs',
    'MAX_FONT_FAMILY_BYTES': 'typography.rs',
    'MIN_FONT_SIZE': 'typography.rs',
    'MAX_FONT_SIZE': 'typography.rs',
    'FontProfile': 'typography.rs',
    'LigaturePolicy': 'typography.rs',
    'MAX_LIGATURE_FEATURES_PER_KIND': 'typography.rs',
    'MAX_LIGATURE_RAW_FEATURE_BYTES': 'typography.rs',
    'MAX_LIGATURE_FEATURE_NAME_BYTES': 'typography.rs',
    'FontProfileValidationError': 'typography.rs',
    'is_generic_font_family': 'typography.rs',
    'ActiveTypography': 'typography.rs',
    'UiTypographyHierarchy': 'typography.rs',
    'HIERARCHY_SCALE_MIN': 'typography.rs',
    'HIERARCHY_SCALE_MAX': 'typography.rs',
    'UiTypographyHierarchyValidationError': 'typography.rs',
    'ActiveTypographyValidationError': 'typography.rs',
    # document.rs
    'DocumentTextHead': 'document.rs',
    'DocumentChunkRejection': 'document.rs',
    'bounded_document_chunk_bytes': 'document.rs',
    'DocumentMetadata': 'document.rs',
    'FileErrorCode': 'document.rs',
    'DocumentAccess': 'document.rs',
    # launcher.rs
    'LauncherWorkspaceEntry': 'launcher.rs',
    'LauncherAgentEntry': 'launcher.rs',
    'LauncherEntries': 'launcher.rs',
    # theme.rs
    'ActiveTheme': 'theme.rs',
    'Appearance': 'theme.rs',
    'ResolvedAppearance': 'theme.rs',
    'WireDesignTokenValue': 'theme.rs',
    'UiDesignTokenOverride': 'theme.rs',
    # shell.rs
    'ShellPreferences': 'shell.rs',
    'TabEntry': 'shell.rs',
    'TabRegistrySnapshot': 'shell.rs',
    'TabCommand': 'shell.rs',
    # messages.rs
    'ClientMessage': 'messages.rs',
    'ServerMessage': 'messages.rs',
    'ProtocolErrorCode': 'messages.rs',
    'RegionLockConflict': 'messages.rs',
    'LockOwner': 'messages.rs',
    'EditRejection': 'messages.rs',
    # existing families
    'AgentSettingsFileInfo': 'agent.rs',
    'DiagnosticSeverity': 'diagnostics.rs',
    'RuntimeDiagnostic': 'diagnostics.rs',
    # keep in mod.rs
    'PROTOCOL_VERSION': None,
    'PerformanceTraceId': None,
    'ViewportRequestId': None,
    'ClientId': None,
    'DocumentId': None,
    'DocumentVersion': None,
    'TabId': None,
    'BehaviorVersion': None,
    'TransactionId': None,
    'LeaseId': None,
    'RegionLockId': None,
    'WorkspaceRootId': None,
    'menu_session_id_serde': None,
}

DECL = re.compile(r'^(pub |pub\(crate\) )?(struct|enum|type|fn|const|static|impl|trait|mod|union) ')
IMPL = re.compile(r'^impl(?:<[^>]*>)? (?:Default for )?([A-Za-z_][A-Za-z0-9_]*)')
DECL_NAME = re.compile(
    r'^(?:pub |pub\(crate\) )?(?:struct|enum|type|fn|const|static|trait|mod|union) '
    r'([A-Za-z_][A-Za-z0-9_]*)'
)
ATTR_OR_DOC = re.compile(r'^\s*(#\[|///|//!|//[^/])')


def parse_items(lines):
    """Top-level items with their contiguous attribute/doc-comment prefix."""
    items = []
    pending = None
    depth = 0  # square-bracket depth inside a multi-line attribute
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
        if s.startswith('///') or s.startswith('//!'):
            if pending is None:
                pending = i
            continue
        if not s:
            pending = None
            continue
        if DECL.match(line):
            if line.startswith('pub mod ') and line.rstrip().endswith(';'):
                pending = None
                continue
            start = i if pending is None else pending
            m = DECL_NAME.match(line)
            name = m.group(1) if m else (IMPL.match(line).group(1) if IMPL.match(line) else None)
            items.append((name, start, i, line))
        pending = None
    return items


def main():
    lines = open(SRC).read().splitlines()
    items = parse_items(lines)
    # Block for item k runs from its `start` (incl. attrs/doc) to the next
    # item's `start`; trailing blank lines are trimmed.
    blocks = []
    for k, (name, start, decl_i, line) in enumerate(items):
        end = items[k + 1][1] if k + 1 < len(items) else len(lines)
        block = lines[start:end]
        while block and not block[-1].strip():
            block.pop()
        blocks.append((name, block, start, end))

    unmatched = sorted({n for n, _, _, _ in blocks if n not in ASSIGN})
    if unmatched:
        print('UNMATCHED ITEMS:', unmatched)
        sys.exit(1)

    # spans must be strictly increasing and non-overlapping
    for a, b in zip(blocks, blocks[1:]):
        if b[2] < a[3]:
            print('OVERLAP:', a[0], a[2], a[3], 'vs', b[0], b[2], b[3])
            sys.exit(1)

    # coverage report: every line outside the header (0..26) and the last item must be covered
    covered = set()
    for _, _, s, e in blocks:
        covered.update(range(s, e))
    gaps = [i + 1 for i in range(len(lines)) if i not in covered and i > 25 and lines[i].strip()]
    if gaps:
        print('UNCOVERED LINES:', gaps[:40], '...' if len(gaps) > 40 else '')
        sys.exit(1)

    per_file = {}
    kept = []
    positions = {}
    for name, block, s, e in blocks:
        dest = ASSIGN[name]
        if dest is None:
            kept.append((name, block, s))
            positions[name] = s
        else:
            per_file.setdefault(dest, []).append(block)

    if DRY:
        for dest, bl in sorted(per_file.items()):
            total = sum(len(b) for b in bl)
            print(f'{dest:18s} items={len(bl):3d} lines={total}')
        print('KEEP:', [n for n, _, _ in kept])
        return

    # new files
    for fname, doc in NEW_FILES.items():
        parts = per_file.pop(fname, [])
        body_parts = [doc.rstrip(), 'use super::*;']
        for b in parts:
            body_parts.append('\n'.join(b))
        if fname == 'behavior.rs':
            # keep the moved test module last (it is already #[cfg(test)]-gated)
            body_parts = [
                doc.rstrip(),
                'use super::*;',
                '\n\n'.join('\n'.join(b) for b in parts),
            ]
        content = '\n\n'.join(body_parts) + '\n'
        open(f'src/protocol/{fname}', 'w').write(content)
        print(f'wrote src/protocol/{fname} ({content.count(chr(10))} lines)')

    # existing files: insert before the first #[cfg(test)] line
    for fname in ('agent.rs', 'diagnostics.rs'):
        parts = per_file.pop(fname, [])
        if not parts:
            continue
        path = f'src/protocol/{fname}'
        existing = open(path).read().splitlines()
        idx = next(i for i, l in enumerate(existing) if l.startswith('#[cfg(test)]'))
        insert = '\n\n'.join('\n'.join(b) for b in parts)
        merged = existing[:idx] + [insert, ''] + existing[idx:]
        open(path, 'w').write('\n'.join(merged) + '\n')
        print(f'updated src/protocol/{fname} (inserted {len(parts)} item(s) at line {idx + 1})')

    if per_file:
        print('LEFTOVER:', {k: len(v) for k, v in per_file.items()})
        sys.exit(1)

    # new mod.rs
    mods = [
        'agent', 'behavior', 'caret', 'codec', 'completion', 'decorations',
        'diagnostics', 'document', 'editor_control', 'editor_rules', 'folding',
        'language_intelligence', 'launcher', 'menu', 'messages', 'parse',
        'runtime', 'sdui', 'shell', 'textobjects', 'theme', 'typography',
    ]
    header = [f'pub mod {m};' for m in mods] + [''] + [f'pub use {m}::*;' for m in mods]
    out = list(header)
    for name, block, s in kept:
        out.append('')
        out.extend(block)
    open(SRC, 'w').write('\n'.join(out) + '\n')
    print(f'wrote {SRC} ({len(out)} lines)')


main()