#!/usr/bin/env python3
"""Plan 133 task 4 verification: tests moved verbatim, none lost, helpers used.

For each original `src/**/tests.rs` (kept as /tmp/tests-*.bak) and the generated
`src/**/tests/` tree it checks:

  * every original top-level item (imports, helpers, tests) reappears exactly
    once across the root `tests/mod.rs` + suite files, as the same code
    (whitespace-insensitive, tolerating rustfmt reflow) after the documented
    `super::` / `include_*!` depth bump for the items that moved one module
    deeper;
  * each test fn name is defined in exactly one suite file, per-suite counts sum
    to the original count, and no `#[test]` remains in a root file;
  * every root helper item is referenced from at least one suite (or another
    root helper).

Run from the repository root.
"""
import collections
import os
import re
import sys

CASES = {
    'src/server/js_runtime/tests.rs': '/tmp/tests-js.bak',
    'src/server/connection/tests.rs': '/tmp/tests-conn.bak',
    'src/client/tests.rs': '/tmp/tests-client.bak',
    'src/server/workspace/tests.rs': '/tmp/tests-ws.bak',
}
DECL = re.compile(r'^(?:pub(?:\([a-z]+\))? )?(?:async )?(?:fn|struct|enum|impl|mod|const|static|type|trait|union)\b')
FN = re.compile(r'^(?:pub(?:\([a-z]+\))? )?(?:async )?fn ([A-Za-z_][A-Za-z0-9_]*)')
INCLUDE = re.compile(r'(include(?:_str|_bytes)?!\(\s*")([^"$][^"]*)(")')


def bump(line):
    line = re.sub(r'super::(?:super::)*', lambda m: 'super::' + m.group(0), line)
    return INCLUDE.sub(
        lambda m: m.group(1)
        + (m.group(2) if m.group(2).startswith('/') else '../' + m.group(2))
        + m.group(3),
        line,
    )


def norm(text):
    # Whitespace-insensitive; trailing commas drop out because rustfmt may move
    # them between `f(arg,)` and `f(arg),` when re-wrapping.
    return re.sub(r',(?=[)\]}])', '', re.sub(r'\s+', '', text))


def items(lines):
    """Top-level items with their contiguous attribute/comment prefix (0-based)."""
    blocks, pending, depth = [], None, 0
    for i, line in enumerate(lines):
        s = line.strip()
        if depth > 0:
            depth += s.count('[') - s.count(']')
            continue
        if s.startswith('#['):
            pending = i if pending is None else pending
            depth += s.count('[') - s.count(']')
            continue
        if s.startswith('//'):
            pending = i if pending is None else pending
            continue
        if not s:
            pending = None
            continue
        if DECL.match(line):
            start = pending if pending is not None else i
            m = FN.match(s)
            blocks.append({'name': m.group(1) if m else None, 'start': start, 'decl': i,
                           'attrs': ' '.join(x.strip() for x in lines[start:i]),
                           'text': s})
        pending = None
    for k, b in enumerate(blocks):
        end = blocks[k + 1]['start'] if k + 1 < len(blocks) else len(lines)
        while end > b['decl'] and not lines[end - 1].strip():
            end -= 1
        b['end'] = end
    return blocks


failures = 0
for path, backup in CASES.items():
    original = open(backup).read().split('\n')
    root = open(path.replace('tests.rs', 'tests/mod.rs')).read().split('\n')
    suite_dir = path.replace('tests.rs', 'tests')
    suites = {
        name[:-3]: open(os.path.join(suite_dir, name)).read().split('\n')
        for name in sorted(os.listdir(suite_dir))
        if name != 'mod.rs' and name.endswith('.rs')
    }
    blocks = items(original)
    tests = [b for b in blocks if '#[test]' in b['attrs'] or '#[tokio::test]' in b['attrs']]
    helpers = [b for b in blocks if b not in tests and not b['text'].startswith('use ')]

    # every original item must reappear exactly once, unchanged (modulo bump)
    output_items = []  # (owner label, name, normalized text)
    for label, lines in [('tests/mod.rs', root)] + sorted(suites.items()):
        for b in items(lines):
            if b['text'].startswith('#[cfg') or b['text'].startswith('mod '):
                continue
            output_items.append((label, b['name'], norm('\n'.join(lines[b['start']:b['end']]))))
    by_name = collections.defaultdict(list)
    for label, name, text in output_items:
        by_name[name].append((label, text))

    problems = []
    per_suite = collections.Counter()
    for b in blocks:
        text = norm('\n'.join(
            (bump(l) if b in tests else l) for l in original[b['start']:b['end']]
        ))
        owners = [label for label, t in by_name[b['name']] if t == text]
        if len(owners) != 1:
            same_name = [label for label, _ in by_name[b['name']]]
            problems.append(f'{b["name"]}: {len(owners)} exact matches (same-name files: {same_name})')
            continue
        if b in tests:
            per_suite[owners[0]] += 1
    root_tests = sum(1 for l in root if l.strip() in ('#[test]', '#[tokio::test]'))
    if root_tests:
        problems.append(f'root still declares {root_tests} #[test] items')
    if sum(per_suite.values()) != len(tests):
        problems.append(f'{sum(per_suite.values())} of {len(tests)} tests placed')

    unreferenced = []
    for b in helpers:
        name = b['name']
        if not name:
            continue
        haystacks = [norm('\n'.join(lines)) for lines in suites.values()]
        haystacks += [
            norm('\n'.join(original[h['start']:h['end']]))
            for h in helpers if h is not b
        ]
        if not any(re.search(r'(?<![\w])' + re.escape(name) + r'(?![\w])', h)
                   for h in haystacks):
            unreferenced.append(name)

    ok = not problems and not unreferenced
    detail = ', '.join(f'{n}:{c}' for n, c in sorted(per_suite.items()))
    print(f'{"ok  " if ok else "FAIL"} {path}: {len(tests)} tests -> {len(suites)} suites '
          f'({detail}); helpers {len(helpers)}, unreferenced '
          f'{unreferenced if unreferenced else "none"}; root tests {root_tests}')
    for p in problems:
        print('     -', p)
    failures += 0 if ok else 1

print('FAILURES:', failures)
sys.exit(1 if failures else 0)