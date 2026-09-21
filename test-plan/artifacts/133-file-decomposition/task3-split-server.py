#!/usr/bin/env python3
"""Plan 133 task 3: split src/server/mod.rs (plan 133 task 3).

Moves, verbatim, into focused submodules:

  src/server/runtime_state.rs            stores/config/fanout + runtime outputs
  src/server/runtime_reload.rs           IpcServer construction + hot reload
  src/server/runtime_outputs_tests.rs    } inline unit-test suites extracted from
  src/server/runtime_generation_tests.rs } mod.rs (they exercise pub(crate)/
  src/server/tab_server_state_tests.rs   } private items, so they stay unit tests)
  src/server/windows_tests.rs

Kept in mod.rs: module decls, IpcServer struct, tab-state plumbing, accept loops,
spawn_connection, socket binding/validation, ServerError, LiveClientGuard.
Run from the repository root.
"""
import re
import sys

SRC = 'src/server/mod.rs'
DRY = '--dry' in sys.argv
PROD_LIMIT = 2670  # first inline `#[cfg(test)] mod` sits at line 2681

DECL = re.compile(r'^(pub(\([a-z]+\))? )?(async )?(fn|struct|enum|impl|mod|const|static|type|trait|union)\b')
METHOD = re.compile(r'^    (pub(\([a-z]+\))? )?(async )?fn \w')
DECL_NAME = re.compile(
    r'^(?:pub(?:\([a-z]+\))? )?(?:async )?(?:struct|enum|type|fn|const|static|trait|mod|union) '
    r'([A-Za-z_][A-Za-z0-9_]*)'
)
IMPL_NAME = re.compile(r'^impl(?:<[^>]*>)? (?:(?:std::fmt::|Default for )?)([A-Za-z_][A-Za-z0-9_]*)')


def collect(lines, lo, hi, pred):
    """Items with their contiguous attribute/doc-comment prefix (start index)."""
    blocks = []
    pending = None
    depth = 0
    for i in range(lo, hi):
        s = lines[i].strip()
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
        if pred(lines[i]):
            start = pending if pending is not None else i
            decl_text = lines[i].strip()
            name = None
            m = DECL_NAME.match(decl_text)
            if m:
                name = m.group(1)
            elif decl_text.startswith('impl'):
                m = IMPL_NAME.match(decl_text)
                name = m.group(1) if m else decl_text
            blocks.append({'name': name, 'start': start, 'decl': i, 'text': lines[i]})
        pending = None
    for k, b in enumerate(blocks):
        end = blocks[k + 1]['start'] if k + 1 < len(blocks) else hi
        while end > b['decl'] and not lines[end - 1].strip():
            end -= 1
        b['end'] = end
    return blocks


lines = open(SRC).read().split('\n')
top = collect(lines, 0, PROD_LIMIT, lambda l: bool(DECL.match(l)))
by_name = {}
for b in top:
    by_name.setdefault(b['name'], []).append(b)
impl_blocks = [b for b in top if b['text'].startswith('impl ')]


def methods_of(impl):
    return collect(lines, impl['decl'] + 1, impl['end'] - 1, lambda l: bool(METHOD.match(l)))


RUNTIME_STATE_ITEMS = [
    'ServerConfig', 'RuntimeGeneration', 'RuntimeGenerationStore',
    'ActiveRuntimeStateFanout', 'ActiveTypographyState', 'RuntimeOutputApplication',
    'apply_runtime_outputs', 'apply_runtime_outputs_without_sdui', 'shell_command_catalogue',
]
RUNTIME_RELOAD_ITEMS = [
    'effective_agent_root', 'RuntimeGenerationCandidate',
    'ReloadedDocumentRefresh', 'RuntimeReloadOutcome', 'register_runtime_contributions',
    'cancel_older_runtime_generations', 'withdraw_package_contributions', 'stage_typography',
    'build_runtime_state_snapshot', 'runtime_candidate_error',
]
RUNTIME_RELOAD_METHODS = [
    'new', 'try_new', 'load_default_configuration', 'load_configuration_for_service',
    'record_runtime_error', 'record_configuration_diagnostics', 'record_runtime_diagnostic',
    'trigger_developer_hot_reload', 'arm_reload_candidate_barrier', 'execute_reload_command',
    'reload_runtime_generation', 'reload_runtime_generation_inner',
    'prepare_runtime_generation_candidate', 'validate_runtime_registrations',
    'commit_runtime_generation', 'refresh_open_documents_after_reload', 'enumerate_ui_choices',
]

# (file, attr line, mod decl line, closing brace line) — 1-based, verified against
# the current file; the mod bodies contain a column-0 JS fixture raw string that
# confuses naive top-level parsing, so they are pinned explicitly.
TEST_MODS = [
    ('runtime_outputs_tests.rs', 2681, 2682, 2911, 'runtime_outputs_tests'),
    ('runtime_generation_tests.rs', 2913, 2914, 5990, 'runtime_generation_tests'),
    ('tab_server_state_tests.rs', 5992, 5993, 6129, 'tab_server_state_tests'),
    ('windows_tests.rs', 6134, 6135, 6203, 'windows_tests'),
]

ranges = []
state_items, reload_items, reload_methods = [], [], []

for name in RUNTIME_STATE_ITEMS:
    hits = by_name.get(name, [])
    if not hits:
        print('MISSING (state):', name)
        sys.exit(1)
    state_items.extend(hits)
    ranges.extend((b['start'], b['end']) for b in hits)

for name in RUNTIME_RELOAD_ITEMS:
    hits = by_name.get(name, [])
    if not hits:
        print('MISSING (reload):', name)
        sys.exit(1)
    reload_items.extend(hits)
    ranges.extend((b['start'], b['end']) for b in hits)

method_index = {}
for impl in impl_blocks:
    for m in methods_of(impl):
        method_index.setdefault((impl['name'], m['name']), []).append(m)

for name in RUNTIME_RELOAD_METHODS:
    hits = method_index.get(('IpcServer', name), [])
    if not hits:
        print('MISSING (method):', name)
        sys.exit(1)
    reload_methods.extend(hits)
    ranges.extend((b['start'], b['end']) for b in hits)

test_bodies = []
for fname, attr, decl, close, modname in TEST_MODS:
    assert lines[attr - 1].strip() in ('#[cfg(test)]', '#[cfg(all(test, windows))]'), lines[attr - 1]
    assert lines[decl - 1].startswith(f'mod {modname} {{'), lines[decl - 1]
    assert lines[close - 1] == '}', repr(lines[close - 1])
    ranges.append((attr - 1, close))
    test_bodies.append((fname, attr - 1, decl - 1, close - 1))

ranges.sort()
if '--dump-ranges' in sys.argv:
    for a, b in ranges:
        print(f'{a}:{b}')
    sys.exit(0)
if '--list' in sys.argv:
    for b in sorted(state_items + reload_items + reload_methods, key=lambda x: x['start']):
        print(f"{b['start'] + 1:5d}-{b['end']:5d}  {str(b['name']):28s} {b['text'].strip()[:60]}")
for a, b in zip(ranges, ranges[1:]):
    if b[0] < a[1]:
        print('OVERLAP:', a, b)
        sys.exit(1)

print('moves:')
for label, bl in (('runtime_state.rs', state_items), ('runtime_reload.rs', reload_items),
                  ('runtime_reload.rs (methods)', reload_methods)):
    tot = sum(b['end'] - b['start'] for b in bl)
    print(f'  {label:28s} {len(bl):3d} blocks / {tot:5d} lines: ' +
          ', '.join(str(b['decl'] + 1) for b in sorted(bl, key=lambda x: x['start'])[:6]) + ' ...')
for fname, attr, decl, close in test_bodies:
    print(f'  {fname:28s} lines {attr + 1}-{close + 1}')
removed = sum(e - s for s, e in ranges)
print(f'removing {removed} lines; mod.rs keeps {len(lines) - 1 - removed} lines')

if DRY:
    sys.exit(0)

headers = {
    'runtime_state.rs': (
        '//! Server-owned runtime state: configuration, the live JS runtime\n'
        '//! generation store, the behavior/typography runtime fanouts, and the\n'
        '//! shared application of runtime outputs to server state.'
    ),
    'runtime_reload.rs': (
        '//! `IpcServer` construction and the persistent-runtime reload pipeline:\n'
        '//! configuration load, runtime generation candidate preparation, install,\n'
        '//! and open-document refresh.'
    ),
}


def render(bl):
    return '\n\n'.join('\n'.join(lines[b['start']:b['end']]) for b in sorted(bl, key=lambda x: x['start']))


state_text = '\n'.join([headers['runtime_state.rs'], '', 'use super::*;', '', render(state_items)])
reload_text = '\n'.join([headers['runtime_reload.rs'], '', 'use super::*;', '', render(reload_items), '',
                         'impl IpcServer {', render(reload_methods), '}'])
open('src/server/runtime_state.rs', 'w').write(state_text + '\n')
open('src/server/runtime_reload.rs', 'w').write(reload_text + '\n')
print(f"wrote src/server/runtime_state.rs ({state_text.count(chr(10)) + 1} lines)")
print(f"wrote src/server/runtime_reload.rs ({reload_text.count(chr(10)) + 1} lines)")

for fname, attr, decl, close in test_bodies:
    body = []
    for line in lines[decl + 1:close]:
        body.append(line[4:] if line.startswith('    ') else line)
    open(f'src/server/{fname}', 'w').write('\n'.join(body) + '\n')
    print(f'wrote src/server/{fname} ({len(body)} lines)')

keep = [True] * len(lines)
for s, e in ranges:
    for i in range(s, e):
        keep[i] = False

inserts = {}
first_test = min(b[1] for b in test_bodies)
inserts[first_test] = [
    '#[cfg(test)] mod runtime_outputs_tests;',
    '#[cfg(test)] mod runtime_generation_tests;',
    '#[cfg(test)] mod tab_server_state_tests;',
]
inserts[test_bodies[-1][1]] = ['#[cfg(all(test, windows))] mod windows_tests;']

out = []
for i, line in enumerate(lines):
    if i in inserts:
        out.extend(inserts[i])
    if keep[i]:
        out.append(line)
while len(out) > 1 and not out[-1].strip():
    out.pop()
open(SRC, 'w').write('\n'.join(out) + '\n')
print(f'wrote {SRC} ({len(out)} lines)')
