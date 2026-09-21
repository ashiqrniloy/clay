#!/usr/bin/env python3
"""Plan 133 task 6 (U5): replace hand-written `parse`/`as_str` impls with
`string_enum_impl!` invocations.

Extracts the (variant, string) pairs from the *current* hand-written tables,
verifies that `parse` and `as_str` agree and that they cover exactly the enum's
variants, then rewrites the impl block into the macro invocation (keeping any
other methods in a residual impl). Also emits the golden round-trip test body
and a human-readable map.

Run:  python3 task6-string-enum.py --dry-run | --apply
"""

import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parents[3]

TARGETS = [
    ("src/packages/extension_points.rs", "RelationOperation"),
    ("src/packages/extension_points.rs", "ExtensionContributionKind"),
    ("src/packages/self_update.rs", "ChannelKind"),
    ("src/perf/fixtures.rs", "FixtureKind"),
    ("src/protocol/theme.rs", "Appearance"),
    ("src/shell/components.rs", "ComponentKind"),
    ("src/shell/theme.rs", "ThemeTokenType"),
    ("src/shell/theme.rs", "ElevationLevel"),
    ("src/shell/theme.rs", "ZLevel"),
    ("src/shell/theme.rs", "DensityLevel"),
    ("src/shell/design_system.rs", "BorderStyle"),
    ("src/shell/design_system.rs", "OutlineStyle"),
    ("src/shell/design_system.rs", "TransitionTiming"),
    ("src/shell/design_system.rs", "TransformPreset"),
    ("src/shell/design_system.rs", "RecipeState"),
]

USE_LINE = "use crate::str_enum::string_enum_impl;"


# ---------------------------------------------------------------- rust lexing


def skip_lexeme(text, i):
    """Return the index just past a string/char/comment starting at i, else i."""
    two = text[i : i + 2]
    if two == "//":
        j = text.find("\n", i)
        return len(text) if j < 0 else j
    if two == "/*":
        depth, j = 1, i + 2
        while j < len(text) and depth:
            if text.startswith("/*", j):
                depth += 1
                j += 2
            elif text.startswith("*/", j):
                depth -= 1
                j += 2
            else:
                j += 1
        return j
    raw = re.match(r'r#*"', text[i:])
    if raw:
        hashes = raw.group(0)[1:]
        end = text.find('"' + hashes, i + len(raw.group(0)))
        return len(text) if end < 0 else end + len(hashes) + 1
    if text[i] == '"':
        j = i + 1
        while j < len(text):
            if text[j] == "\\":
                j += 2
                continue
            if text[j] == '"':
                return j + 1
            j += 1
        return len(text)
    if text[i] == "'":
        # char literal or lifetime: only skip a real char literal
        m = re.match(r"'(?:\\.|[^\\'])'", text[i:])
        if m:
            return i + len(m.group(0))
    return i


def find_matching(text, open_idx, open_ch="{"):
    assert text[open_idx] == open_ch
    close_ch = {"{": "}", "(": ")", "[": "]"}[open_ch]
    depth, i = 0, open_idx
    while i < len(text):
        j = skip_lexeme(text, i)
        if j != i:
            i = j
            continue
        c = text[i]
        if c == open_ch:
            depth += 1
        elif c == close_ch:
            depth -= 1
            if depth == 0:
                return i
        i += 1
    raise ValueError("unbalanced braces")


def line_start(text, i):
    return text.rfind("\n", 0, i) + 1


def line_end(text, i):
    j = text.find("\n", i)
    return len(text) if j < 0 else j + 1


def extend_back_over_docs(text, start):
    """Walk start backwards over doc comments / attributes / blank lines."""
    while True:
        ls = line_start(text, start - 1) if start > 0 else 0
        line = text[ls:start]
        stripped = line.strip()
        if stripped.startswith("///") or stripped.startswith("#["):
            start = ls
        else:
            return start


# ------------------------------------------------------------- enum + impl


def enum_variants(text, name):
    m = re.search(r"(?m)^(?:pub(?:\([^)]*\))?\s+)?enum\s+" + name + r"\s*\{", text)
    if not m:
        raise ValueError(f"{name}: enum declaration not found")
    body = text[m.end() : find_matching(text, m.end() - 1)]
    variants = []
    for line in body.split("\n"):
        s = line.strip()
        if not s or s.startswith("//") or s.startswith("#["):
            continue
        vm = re.match(r"([A-Z]\w*)\s*,?$", s)
        if not vm:
            raise ValueError(f"{name}: non-unit variant line {s!r}")
        variants.append(vm.group(1))
    return variants


def find_impl(text, name):
    for m in re.finditer(r"(?m)^impl\s+" + name + r"\s*\{", text):
        end = find_matching(text, m.end() - 1)
        body = text[m.end() : end]
        if "fn as_str(" in body and "fn parse(" in body:
            return m.start(), m.end() - 1, end, body
    raise ValueError(f"{name}: impl with as_str + parse not found")


def find_fn(text, body_off, body, fname):
    m = re.search(r"(?m)^\s*(?:pub(?:\([^)]*\))?\s+)?(?:const\s+)?fn\s+" + fname + r"\s*\(", body)
    if not m:
        return None
    sig_start = body_off + m.start()
    brace = body.find("{", m.end())
    end = find_matching(text, body_off + brace)
    return sig_start, end + 1


def parse_impl(text, name):
    impl_start, impl_brace, impl_end, body = find_impl(text, name)
    variants = enum_variants(text, name)
    fns = {}
    for fname in ("as_str", "parse", "all_as_str"):
        span = find_fn(text, impl_brace + 1, body, fname)
        if span:
            fns[fname] = span
    if "as_str" not in fns or "parse" not in fns:
        raise ValueError(f"{name}: as_str/parse not both inside one impl")

    pairs, comments, as_str_vis, parse_vis = extract_pairs(text, fns, name)
    if [v for v, _ in pairs] != variants:
        raise ValueError(
            f"{name}: impl order {[v for v, _ in pairs]} != enum order {variants}"
        )

    # docs that belonged to the generated methods, carried onto the invocation
    method_docs, method_attrs = [], []
    for fname in ("parse", "as_str"):
        if fname not in fns:
            continue
        start = extend_back_over_docs(text, fns[fname][0])
        for line in text[start : fns[fname][0]].split("\n"):
            stripped = line.strip()
            if stripped.startswith("///"):
                method_docs.append(stripped)
            elif stripped.startswith("#["):
                method_attrs.append(stripped)

    # impl-level doc/attr prefix
    prefix_start = extend_back_over_docs(text, impl_start)
    prefix = text[prefix_start:impl_start]
    return {
        "name": name,
        "pairs": pairs,
        "method_docs": method_docs,
        "method_attrs": method_attrs,
        "comments": comments,
        "fns": fns,
        "impl_prefix": prefix,
        "impl_start": prefix_start,
        "impl_end": impl_end,
        "as_str_vis": as_str_vis,
        "parse_vis": parse_vis,
        "all_as_str": "all_as_str" in fns,
    }


def extract_pairs(text, fns, name):
    """(variant, string) pairs from `as_str`, cross-checked against `parse`."""
    as_str_src = text[fns["as_str"][0] : fns["as_str"][1]]
    parse_src = text[fns["parse"][0] : fns["parse"][1]]
    vis = r"((?:pub(?:\([^)]*\))?\s+)?)(?:const\s+)?fn\s+as_str"
    m = re.search(vis, as_str_src)
    as_str_vis = (m.group(1) or "").strip()
    m = re.search(r"((?:pub(?:\([^)]*\))?\s+)?)fn\s+parse", parse_src)
    parse_vis = (m.group(1) or "").strip()

    def strip_comments(src):
        out = []
        i = 0
        while i < len(src):
            j = skip_lexeme(src, i)
            if j != i and src.startswith(("//", "/*"), i):
                out.append(" " * (j - i))
                i = j
                continue
            out.append(src[i])
            i += 1
        return "".join(out)

    pairs, comments = [], []
    for line in strip_comments(as_str_src).split("\n"):
        s = line.strip()
        am = re.match(r'(?:\w+::)?(\w+)\s*=>\s*"([^"]+)"\s*,?$', s)
        if am:
            pairs.append((am.group(1), am.group(2)))
    as_str_map = dict(pairs)

    parse_map = {}
    arm_res = (
        re.compile(r'"([^"]+)"\s*=>\s*Some\((?:Self::|\w+::)?(\w+)\)\s*,?$'),
        re.compile(r'"([^"]+)"\s*=>\s*(?:Self::|\w+::)?(\w+)\s*,?$'),
    )
    for line in strip_comments(parse_src).split("\n"):
        s = line.strip()
        if s.startswith("_ =>") or s.startswith('_ => return'):
            continue
        for rx in arm_res:
            pm = rx.match(s)
            if pm:
                parse_map[pm.group(1)] = pm.group(2)
                break
    if parse_map != {text_: variant for variant, text_ in pairs}:
        raise ValueError(f"{name}: parse/as_str tables disagree: {parse_map}")

    # the parse body must be *only* the table: no trimming/normalising statements
    arm_re = re.compile(r'^"([^"]+)"\s*=>\s*(?:Some\()?(?:Self::|\w+::)?\w+\)?\s*,?$')
    table = strip_comments(parse_src)
    table = table[table.index("match") :]  # skip the signature
    for line in table.split("\n"):
        s = line.strip()
        if not s or arm_re.match(s):
            continue
        if s in ("match value {", "match raw {", "match s {", "match segment {",
                 "match kind {", "Some(match value {", "Some(match raw {",
                 "Some(match s {", "_ => None", "_ => None,", "}", "})", "})", "_ => return None,"):
            continue
        if s.startswith(("pub fn parse", "fn parse", "//")):
            continue
        if s.startswith(("pub fn parse", "fn parse")):
            continue
        raise ValueError(f"{name}: unexpected statement inside parse body: {s!r}")

    # carry per-variant comments from the (removed) parse body
    pending = []
    for line in parse_src.split("\n"):
        s = line.strip()
        if s.startswith("//"):
            pending.append(line)
            continue
        if s.startswith('"'):
            text_ = re.match(r'"([^"]+)"', s)
            if text_ and pending:
                comments.append((text_.group(1), list(pending)))
            pending = []
        elif s and not s.startswith(("match", "}", "_", "Some(", ")", "fn ", "pub", "}")):
            if not s.startswith(("pub", "fn")):
                pending = []
    return pairs, comments, as_str_vis, parse_vis


# ------------------------------------------------------------------ output


def invocation(spec, indent=""):
    prefix = spec["impl_prefix"].strip("\n")
    docs = prefix.split("\n") if prefix.strip() else []
    attrs = spec.get("method_attrs", [])
    attrs += [d.strip() for d in docs if d.strip().startswith("#[") and d.strip() not in attrs]
    method_docs = spec.get("method_docs", []) + [line.strip() for line in docs if line.strip().startswith("///")]
    out = []
    vis = spec["as_str_vis"]
    suffix = ", parse_private" if not spec["parse_vis"] else ""
    out.append(f"{indent}string_enum_impl! {{")
    out += [indent + "    " + d for d in method_docs]
    out += [indent + "    " + a for a in attrs]
    out.append(f"{indent}    {vis} {spec['name']}{suffix} {{")
    by_text = dict(spec["comments"])
    for variant, text_ in spec["pairs"]:
        out += [indent + "        " + c.strip() for c in by_text.get(text_, [])]
        out.append(f'{indent}        {variant} => "{text_}",')
    out.append(f"{indent}    }}")
    if spec["all_as_str"]:
        out.append(f"{indent}    all_as_str")
    out.append(f"{indent}}}")
    return "\n".join(out)


def residual_impl(spec, text, indent=""):
    """Impl text that survives: remove the generated fns (with their docs),
    keep every other method."""
    fns = spec["fns"]
    cut = []
    for key in ("as_str", "parse", "all_as_str"):
        if key not in fns:
            continue
        start, end = fns[key]
        cut.append((extend_back_over_docs(text, start), end))
    cut.sort()
    body_start, body_end = spec["impl_start"], spec["impl_end"] + 1  # include closing brace
    chunks, prev = [], body_start
    for start, end in cut:
        chunks.append(text[prev:start])
        prev = end
    chunks.append(text[prev:body_end])
    # guarantee a line boundary wherever a removal glued two chunks together
    out = chunks[0]
    for chunk in chunks[1:]:
        if out and not out.endswith("\n") and chunk and not chunk.startswith("\n"):
            out += "\n"
        out += chunk
    out = re.sub(r"\n{3,}", "\n\n", out)
    out = re.sub(r"\n[ \t]*\n\}", "\n}", out)
    if "{" not in out:
        return ""
    body = out[out.index("{") + 1 :]
    return out if "fn " in body else ""


def main():
    dry = "--apply" not in sys.argv
    report = []
    test_calls = []
    per_file = {}
    for rel, name in TARGETS:
        path = ROOT / rel
        text = path.read_text()
        spec = parse_impl(text, name)
        report.append(
            f"{rel}:{name} vis={spec['as_str_vis']!r} parse_vis={spec['parse_vis']!r} "
            f"all_as_str={spec['all_as_str']} impl_prefix={spec['impl_prefix']!r}"
        )
        for variant, text_ in spec["pairs"]:
            report.append(f'    {variant} => "{text_}"')
        inv = invocation(spec)
        residual = residual_impl(spec, text)
        report.append("    invocation:")
        report += ["    | " + l for l in inv.split("\n")]
        if residual.strip():
            report.append("    residual impl kept:")
            report += ["    | " + l for l in residual.strip("\n").split("\n")]
        new_block = inv + ("\n\n" + residual.strip("\n") if residual.strip() else "")
        new_text = text[: spec["impl_start"]] + new_block + "\n" + text[spec["impl_end"] + 1 :]
        if USE_LINE not in new_text:
            lines = new_text.split("\n")
            for i, line in enumerate(lines):
                if line.startswith("use "):
                    lines.insert(i, USE_LINE + "\n")
                    break
            else:
                lines.insert(0, USE_LINE + "\n")
            new_text = "\n".join(lines)
        per_file.setdefault(rel, []).append((spec["impl_start"], spec["impl_end"], new_text))
        pairs_src = ", ".join(f'({name}::{v}, "{t}")' for v, t in spec["pairs"])
        test_calls.append((name, pairs_src))
        if not dry:
            path.write_text(new_text)
    # write report + golden test calls
    out_dir = pathlib.Path(__file__).resolve().parent
    (out_dir / "task6-string-enum-map.txt").write_text("\n".join(report) + "\n")
    (out_dir / "task6-golden-calls.txt").write_text(
        "\n".join(f'        "{n}", &[{p}],' for n, p in test_calls) + "\n"
    )
    print(f"{'DRY RUN' if dry else 'APPLIED'} — {len(TARGETS)} enums in {len(per_file)} files")
    print("\n".join(report[:40]))


if __name__ == "__main__":
    main()
