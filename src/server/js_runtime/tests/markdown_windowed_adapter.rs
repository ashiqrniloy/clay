use super::*;

#[tokio::test]
async fn markdown_parser_adapter_publishes_viewport_bounded_decorations() {
    let root = config_fixture("markdown-parser-adapter");
    let parser_source = fs::read_to_string("packages/markdown/dist/parser.js")
        .expect("markdown parser adapter must exist");
    fs::write(root.join("parser.js"), parser_source).unwrap();
    fs::write(
        root.join("init.js"),
        r##"
        import { serverPublishDecorations } from "clay:decorations";
        import { parseMarkdownDecorations, publishMarkdownDecorations } from "clay://packages/@clay/markdown-adapter/parser.js";

        const text = "# Hé 🦀\n\nSome **bold** and *em* and `code`.\n\n```js\nx\n```\n\n1. item\n";
        const markdownTokens = [
          { type: "heading_open", tag: "h1", map: [0, 1] },
          { type: "inline", map: [0, 1], content: "Hé 🦀", children: [] },
          { type: "heading_close" },
          { type: "paragraph_open", map: [2, 3] },
          {
            type: "inline",
            map: [2, 3],
            content: "Some **bold** and *em* and `code`.",
            children: [
              { type: "text", content: "Some " },
              { type: "strong_open", markup: "**" },
              { type: "text", content: "bold" },
              { type: "strong_close", markup: "**" },
              { type: "text", content: " and " },
              { type: "em_open", markup: "*" },
              { type: "text", content: "em" },
              { type: "em_close", markup: "*" },
              { type: "text", content: " and " },
              { type: "code_inline", markup: "`", content: "code" }
            ]
          },
          { type: "paragraph_close" },
          { type: "fence", tag: "code", map: [4, 7], markup: "```", info: "js" },
          { type: "ordered_list_open", map: [8, 9] },
          { type: "list_item_open", map: [8, 9] },
          { type: "paragraph_open", map: [8, 9] },
          { type: "inline", map: [8, 9], content: "item", children: [{ type: "text", content: "item" }] },
          { type: "paragraph_close" },
          { type: "list_item_close" },
          { type: "ordered_list_close" }
        ];
        function utf8ByteLength(value) {
          let bytes = 0;
          for (const character of value) {
            const codePoint = character.codePointAt(0);
            bytes += codePoint <= 0x7f ? 1 : codePoint <= 0x7ff ? 2 : codePoint <= 0xffff ? 3 : 4;
          }
          return bytes;
        }
        function byteRangeFor(needle, from = 0) {
          const codeUnitStart = text.indexOf(needle, from);
          if (codeUnitStart < 0) throw new Error(`missing fixture needle: ${needle}`);
          return {
            byteStart: utf8ByteLength(text.slice(0, codeUnitStart)),
            byteEnd: utf8ByteLength(text.slice(0, codeUnitStart + needle.length)),
            codeUnitEnd: codeUnitStart + needle.length,
          };
        }
        function styleToken(span) {
          if (span.styleToken) return span.styleToken;
          const modifiers = span.modifiers ?? [];
          if (span.tokenType === "Heading1") return "markup.heading.1";
          if (span.tokenType === "Paragraph" && modifiers.includes("Bold")) return "markup.strong";
          if (span.tokenType === "Paragraph" && modifiers.includes("Italic")) return "markup.emphasis";
          if (span.tokenType === "CodeSpan") return "markup.inline-code";
          if (span.tokenType === "CodeBlock") return "markup.code-block";
          if (span.tokenType === "ListItem") return "markup.list-marker";
          return undefined;
        }
        function requireSpan(style) {
          const span = spans.find((candidate) => styleToken(candidate) === style);
          if (!span) throw new Error(`missing span ${style} in ${JSON.stringify(spans)}`);
          return span;
        }
        function assertSpan(styleToken, expected) {
          const span = requireSpan(styleToken);
          if (span.byteStart !== expected.byteStart || span.byteEnd !== expected.byteEnd) {
            throw new Error(`${styleToken} expected ${expected.byteStart}:${expected.byteEnd}, got ${span.byteStart}:${span.byteEnd}`);
          }
        }

        const fullViewport = { byteStart: 0, byteEnd: utf8ByteLength(text) };
        const spans = await parseMarkdownDecorations({ text, tokens: markdownTokens, viewport: fullViewport });
        assertSpan("markup.heading.1", { byteStart: 0, byteEnd: utf8ByteLength("# Hé 🦀") });
        assertSpan("markup.strong", byteRangeFor("**bold**"));
        assertSpan("markup.emphasis", byteRangeFor("*em*"));
        assertSpan("markup.inline-code", byteRangeFor("`code`"));
        assertSpan("markup.list-marker", byteRangeFor("1."));
        const fenceStart = byteRangeFor("```js");
        const fenceTerminator = byteRangeFor("\n\n1. item");
        assertSpan("markup.code-block", { byteStart: fenceStart.byteStart, byteEnd: utf8ByteLength(text.slice(0, fenceTerminator.codeUnitEnd - "\n1. item".length)) });
        if (requireSpan("markup.inline-code").fontRole !== "monospace" || requireSpan("markup.code-block").fontRole !== "monospace") {
          throw new Error("Markdown code spans must declare the generic monospace role");
        }

        const listMarker = requireSpan("markup.list-marker");
        const viewportOnlyList = await parseMarkdownDecorations({
          text,
          tokens: markdownTokens,
          viewport: { byteStart: listMarker.byteStart, byteEnd: listMarker.byteEnd },
        });
        if (viewportOnlyList.length !== 1 || styleToken(viewportOnlyList[0]) !== "markup.list-marker") {
          throw new Error(`viewport filter leaked spans: ${JSON.stringify(viewportOnlyList)}`);
        }

        let parseCalls = 0;
        const fakeMarkdownIt = {
          parse(source, env) {
            parseCalls += 1;
            if (source !== text || !env) throw new Error("parse received unexpected arguments");
            return markdownTokens;
          },
          render() {
            throw new Error("adapter must not render HTML");
          }
        };
        await parseMarkdownDecorations({ text, markdownIt: fakeMarkdownIt, viewport: fullViewport });
        if (parseCalls !== 1) throw new Error(`expected one markdown-it parse call, got ${parseCalls}`);

        const tokens = spans.map(styleToken).sort().join(",");
        const heading = requireSpan("markup.heading.1");
        const published = await publishMarkdownDecorations({ decorations: { serverPublishDecorations } }, {
          text,
          tokens: markdownTokens,
          documentId: 7,
          documentVersion: 3,
          behaviorVersion: 2,
          viewport: fullViewport,
        });
        Deno.core.ops.op_clay_runtime_record(tokens);
        Deno.core.ops.op_clay_runtime_record(`${heading.byteStart}:${heading.byteEnd}:${published.publishedSpanCount}:parseCalls=${parseCalls}`);
        "##,
    )
    .unwrap();

    // The adapter publishes through a package context: register the
    // parser module in the load-entry allowlist for a synthetic package
    // and evaluate the script with that package's host-stamped provenance.
    let service = ClayJsRuntimeService::default();
    service
        .test_op_state()
        .load_entry_allowlist()
        .record_for_package(
            "clay://packages/@clay/markdown-adapter/parser.js",
            fs::canonicalize(root.join("parser.js")).unwrap(),
            fs::canonicalize(&root).unwrap(),
            Some("@clay/markdown-adapter"),
        );
    let source = fs::read_to_string(root.join("init.js")).unwrap();
    let result = evaluate_as_package(
        &service,
        test_package_json(
            "@clay/markdown-adapter",
            "markdown",
            &["parse-document", "render-decorations"],
            serde_json::json!({}),
        ),
        vec![
            crate::packages::permissions::PackagePermission::ParseDocument,
            crate::packages::permissions::PackagePermission::RenderDecorations,
        ],
        &source,
    )
    .await
    .unwrap();

    let tokens = &result.op_records[0];
    for expected in [
        "markup.heading.1",
        "markup.strong",
        "markup.emphasis",
        "markup.inline-code",
        "markup.code-block",
        "markup.list-marker",
    ] {
        assert!(tokens.contains(expected), "missing {expected} in {tokens}");
    }
    assert_eq!(result.op_records[1], "0:10:6:parseCalls=1");
    assert_eq!(result.published_decoration_set.unwrap().spans.len(), 6);
}

#[tokio::test]
async fn markdown_windowed_adapter_offsets_ranges_to_absolute_document_bytes() {
    let root = config_fixture("markdown-windowed-absolute-ranges");
    let parser_source = fs::read_to_string("packages/markdown/dist/parser.js")
        .expect("markdown parser adapter must exist");
    fs::write(root.join("parser.js"), parser_source).unwrap();
    fs::write(
        root.join("init.js"),
        r##"
        import { parseMarkdownDecorations } from "./parser.js";

        const windowText = "# Hé 🦀\n\nParagraph **dé** and `cø`.\n";
        const absoluteByteStart = 4096;
        const tokens = [
          { type: "heading_open", tag: "h1", map: [0, 1] },
          { type: "inline", map: [0, 1], content: "Hé 🦀", children: [{ type: "text", content: "Hé 🦀" }] },
          { type: "heading_close" },
          { type: "paragraph_open", map: [2, 3] },
          {
            type: "inline",
            map: [2, 3],
            content: "Paragraph **dé** and `cø`.",
            children: [
              { type: "text", content: "Paragraph " },
              { type: "strong_open", markup: "**" },
              { type: "text", content: "dé" },
              { type: "strong_close", markup: "**" },
              { type: "text", content: " and " },
              { type: "code_inline", markup: "`", content: "cø" },
              { type: "text", content: "." }
            ]
          },
          { type: "paragraph_close" }
        ];
        function utf8ByteLength(value) {
          let bytes = 0;
          for (const character of value) {
            const codePoint = character.codePointAt(0);
            bytes += codePoint <= 0x7f ? 1 : codePoint <= 0x7ff ? 2 : codePoint <= 0xffff ? 3 : 4;
          }
          return bytes;
        }
        function absoluteRangeFor(needle, from = 0) {
          const start = windowText.indexOf(needle, from);
          if (start < 0) throw new Error(`missing ${needle}`);
          return {
            byteStart: absoluteByteStart + utf8ByteLength(windowText.slice(0, start)),
            byteEnd: absoluteByteStart + utf8ByteLength(windowText.slice(0, start + needle.length))
          };
        }
        function styleToken(span) {
          if (span.styleToken) return span.styleToken;
          const modifiers = span.modifiers ?? [];
          if (span.tokenType === "Heading1") return "markup.heading.1";
          if (span.tokenType === "Paragraph" && modifiers.includes("Bold")) return "markup.strong";
          if (span.tokenType === "CodeSpan") return "markup.inline-code";
          return undefined;
        }
        function span(style) {
          const found = spans.find((candidate) => styleToken(candidate) === style);
          if (!found) throw new Error(`missing ${style} in ${JSON.stringify(spans)}`);
          return found;
        }
        function assertRange(styleToken, range) {
          const found = span(styleToken);
          if (found.byteStart !== range.byteStart || found.byteEnd !== range.byteEnd) {
            throw new Error(`${styleToken} expected ${range.byteStart}:${range.byteEnd}, got ${found.byteStart}:${found.byteEnd}`);
          }
        }

        let parseCalls = 0;
        const fakeMarkdownIt = {
          parse(source, env) {
            parseCalls += 1;
            if (source !== windowText || !env) throw new Error("markdown-it must receive only window text");
            return tokens;
          },
          render() {
            throw new Error("windowed adapter must not render HTML");
          }
        };
        const spans = await parseMarkdownDecorations({
          text: windowText,
          absoluteByteStart,
          baseLine: 120,
          parseWindow: { byteStart: absoluteByteStart, byteEnd: absoluteByteStart + utf8ByteLength(windowText), baseLine: 120 },
          viewport: { byteStart: absoluteByteStart, byteEnd: absoluteByteStart + utf8ByteLength(windowText) },
          markdownIt: fakeMarkdownIt
        });

        assertRange("markup.heading.1", { byteStart: absoluteByteStart, byteEnd: absoluteByteStart + utf8ByteLength("# Hé 🦀") });
        assertRange("markup.strong", absoluteRangeFor("**dé**"));
        assertRange("markup.inline-code", absoluteRangeFor("`cø`"));
        Deno.core.ops.op_clay_runtime_record(`${spans.length}:parseCalls=${parseCalls}:${span("markup.strong").byteStart}:${span("markup.inline-code").byteEnd}`);
        "##,
    )
    .unwrap();

    let result = ClayJsRuntimeService::default()
        .load_configuration_from_root(root)
        .await
        .unwrap();

    assert_eq!(result.op_records, vec!["3:parseCalls=1:4118:4135"]);
}

#[tokio::test]
async fn markdown_windowed_adapter_does_not_parse_full_large_document() {
    let root = config_fixture("markdown-windowed-no-full-doc");
    let parser_source = fs::read_to_string("packages/markdown/dist/parser.js")
        .expect("markdown parser adapter must exist");
    fs::write(root.join("parser.js"), parser_source).unwrap();
    fs::write(
        root.join("init.js"),
        r##"
        import { parseMarkdownDecorationUpdate } from "./parser.js";

        const windowText = "# Visible\n\n- item\n";
        const absoluteByteStart = 8 * 1024 * 1024;
        const largeDocumentSentinel = "x".repeat(16 * 1024 * 1024);
        const tokens = [
          { type: "heading_open", tag: "h1", map: [0, 1] },
          { type: "inline", map: [0, 1], content: "Visible", children: [{ type: "text", content: "Visible" }] },
          { type: "heading_close" },
          { type: "bullet_list_open", map: [2, 3] },
          { type: "list_item_open", map: [2, 3] },
          { type: "paragraph_open", map: [2, 3] },
          { type: "inline", map: [2, 3], content: "item", children: [{ type: "text", content: "item" }] },
          { type: "paragraph_close" },
          { type: "list_item_close" },
          { type: "bullet_list_close" }
        ];
        function utf8ByteLength(value) {
          let bytes = 0;
          for (const character of value) {
            const codePoint = character.codePointAt(0);
            bytes += codePoint <= 0x7f ? 1 : codePoint <= 0x7ff ? 2 : codePoint <= 0xffff ? 3 : 4;
          }
          return bytes;
        }
        let parseCalls = 0;
        const fakeMarkdownIt = {
          parse(source) {
            parseCalls += 1;
            if (source === largeDocumentSentinel || source.length !== windowText.length) {
              throw new Error(`received unbounded source length ${source.length}`);
            }
            return tokens;
          }
        };
        const update = await parseMarkdownDecorationUpdate({
          documentId: 7,
          documentVersion: 3,
          behaviorVersion: 2,
          viewport: { byteStart: absoluteByteStart, byteEnd: absoluteByteStart + utf8ByteLength(windowText) },
          parseWindows: [{
            text: windowText,
            byteStart: absoluteByteStart,
            byteEnd: absoluteByteStart + utf8ByteLength(windowText),
            baseLine: 900
          }],
          markdownIt: fakeMarkdownIt
        });
        if (update.spans.length !== 2) throw new Error(`expected heading and list marker spans, got ${JSON.stringify(update.spans)}`);
        Deno.core.ops.op_clay_runtime_record(`${update.viewport.byteStart}:${update.viewport.byteEnd}:${update.spans.length}:parseCalls=${parseCalls}`);
        "##,
    )
    .unwrap();

    let result = ClayJsRuntimeService::default()
        .load_configuration_from_root(root)
        .await
        .unwrap();

    assert_eq!(result.op_records, vec!["8388608:8388626:2:parseCalls=1"]);
}

#[tokio::test]
async fn markdown_windowed_adapter_preserves_fence_and_list_context() {
    let root = config_fixture("markdown-windowed-fence-list-context");
    let parser_source = fs::read_to_string("packages/markdown/dist/parser.js")
        .expect("markdown parser adapter must exist");
    fs::write(root.join("parser.js"), parser_source).unwrap();
    fs::write(
        root.join("init.js"),
        r##"
        import { parseMarkdownDecorations } from "./parser.js";

        const windowText = "```js\nconst visible = 1;\n```\n\n- item\n";
        const absoluteByteStart = 2048;
        const tokens = [
          { type: "fence", tag: "code", map: [0, 3], markup: "```", info: "js" },
          { type: "bullet_list_open", map: [4, 5] },
          { type: "list_item_open", map: [4, 5] },
          { type: "paragraph_open", map: [4, 5] },
          { type: "inline", map: [4, 5], content: "item", children: [{ type: "text", content: "item" }] },
          { type: "paragraph_close" },
          { type: "list_item_close" },
          { type: "bullet_list_close" }
        ];
        function utf8ByteLength(value) {
          let bytes = 0;
          for (const character of value) {
            const codePoint = character.codePointAt(0);
            bytes += codePoint <= 0x7f ? 1 : codePoint <= 0x7ff ? 2 : codePoint <= 0xffff ? 3 : 4;
          }
          return bytes;
        }
        const visibleStart = absoluteByteStart + utf8ByteLength("```js\n");
        const visibleEnd = absoluteByteStart + utf8ByteLength(windowText.slice(0, windowText.indexOf(" item")));
        const spans = await parseMarkdownDecorations({
          text: windowText,
          tokens,
          absoluteByteStart,
          parseWindow: { byteStart: absoluteByteStart, byteEnd: absoluteByteStart + utf8ByteLength(windowText) },
          viewport: { byteStart: visibleStart, byteEnd: visibleEnd }
        });
        const styleToken = (span) => span.styleToken ?? (span.tokenType === "CodeBlock" ? "markup.code-block" : span.tokenType === "ListItem" ? "markup.list-marker" : undefined);
        const fence = spans.find((span) => styleToken(span) === "markup.code-block");
        const list = spans.find((span) => styleToken(span) === "markup.list-marker");
        if (!fence || fence.byteStart !== visibleStart || fence.byteEnd > visibleEnd) {
          throw new Error(`fence span was not clipped to the visible viewport: ${JSON.stringify(spans)}`);
        }
        if (!list || list.byteStart !== visibleEnd - 1 || list.byteEnd !== visibleEnd) {
          throw new Error(`list marker did not survive guard-window parsing: ${JSON.stringify(spans)}`);
        }
        Deno.core.ops.op_clay_runtime_record(`${spans.length}:${fence.byteStart}:${fence.byteEnd}:${list.byteStart}:${list.byteEnd}`);
        "##,
    )
    .unwrap();

    let result = ClayJsRuntimeService::default()
        .load_configuration_from_root(root)
        .await
        .unwrap();

    assert_eq!(result.op_records, vec!["2:2054:2077:2078:2079"]);
}

#[tokio::test]
async fn markdown_large_file_status_reports_windowed_highlighting() {
    let root = config_fixture("markdown-large-file-windowed-status");
    // index.js re-exports from ./load.js (markdownLoadMode fallback entry),
    // so the whole dist module graph must be copied for sdui.js to load.
    for file_name in ["index.js", "sdui.js", "load.js"] {
        fs::write(
            root.join(file_name),
            fs::read_to_string(format!("packages/markdown/dist/{file_name}"))
                .expect("first-party Markdown runtime module must exist"),
        )
        .unwrap();
    }
    fs::write(
        root.join("init.js"),
        r##"
        import { markdownPreviewStatusModel } from "./sdui.js";

        const model = markdownPreviewStatusModel({
          documentByteLength: 16 * 1024 * 1024,
          documentPath: "C:/Users/alice/work/large.md"
        });
        if (model.status.highlightingState !== "windowed") throw new Error(JSON.stringify(model));
        if (model.status.fileTier !== "large") throw new Error(JSON.stringify(model));
        Deno.core.ops.op_clay_runtime_record(`${model.documentPath}:${model.status.parse}:${model.status.decorations}:${model.status.highlightingState}`);
        "##,
    )
    .unwrap();

    let result = ClayJsRuntimeService::default()
        .load_configuration_from_root(root)
        .await
        .unwrap();

    assert_eq!(
        result.op_records,
        vec![
            "large.md:windowed visible syntax current:visible and near-viewport chunks current:windowed"
        ]
    );
}

#[tokio::test]
async fn markdown_large_file_budget_exhaustion_falls_back_to_plain_text() {
    let root = config_fixture("markdown-large-file-plain-text-fallback");
    let parser_source = fs::read_to_string("packages/markdown/dist/parser.js")
        .expect("markdown parser adapter must exist");
    fs::write(root.join("parser.js"), parser_source).unwrap();
    fs::write(
        root.join("init.js"),
        r##"
        import { parseMarkdownDecorationUpdate } from "./parser.js";

        const windowText = "# Visible\n\n- item\n";
        const fakeMarkdownIt = {
          parse() {
            throw new Error("plain-text fallback must not invoke markdown-it");
          }
        };
        const update = await parseMarkdownDecorationUpdate({
          documentId: 9,
          documentVersion: 4,
          behaviorVersion: 2,
          viewport: { byteStart: 0, byteEnd: 18 },
          parseWindows: [{ text: windowText, byteStart: 0, byteEnd: 18, baseLine: 0 }],
          memoryBudgetBytes: 1,
          markdownIt: fakeMarkdownIt
        });
        if (update.spans.length !== 0) throw new Error(`fallback must clear spans: ${JSON.stringify(update.spans)}`);
        if (update.status.highlightingState !== "plain-text-fallback") throw new Error(JSON.stringify(update.status));
        Deno.core.ops.op_clay_runtime_record(`${update.spans.length}:${update.status.highlightingState}:${update.status.reason}`);
        "##,
    )
    .unwrap();

    let result = ClayJsRuntimeService::default()
        .load_configuration_from_root(root)
        .await
        .unwrap();

    assert_eq!(
        result.op_records,
        vec!["0:plain-text-fallback:budget-exceeded"]
    );
}

#[tokio::test]
async fn markdown_degraded_status_contains_no_document_text_or_paths() {
    let root = config_fixture("markdown-degraded-status-sanitized");
    for file_name in ["index.js", "sdui.js", "load.js"] {
        fs::write(
            root.join(file_name),
            fs::read_to_string(format!("packages/markdown/dist/{file_name}"))
                .expect("first-party Markdown runtime module must exist"),
        )
        .unwrap();
    }
    fs::write(
        root.join("init.js"),
        r##"
        import { markdownPreviewStatusModel } from "./sdui.js";

        const model = markdownPreviewStatusModel({
          documentByteLength: 6 * 1024 * 1024,
          parserTimedOut: true,
          documentPath: "C:/Users/alice/secrets/project.md",
          diagnostic: "C:/Users/alice/secrets/project.md first line SECRET_DOCUMENT_TEXT"
        });
        const encoded = JSON.stringify(model);
        for (const forbidden of ["C:/", "Users/alice", "secrets/project.md", "SECRET_DOCUMENT_TEXT"]) {
          if (encoded.includes(forbidden)) throw new Error(`unsanitized status: ${encoded}`);
        }
        if (model.status.highlightingState !== "degraded") throw new Error(encoded);
        Deno.core.ops.op_clay_runtime_record(`${model.documentPath}:${model.status.parse}:${model.status.highlightingState}`);
        "##,
    )
    .unwrap();

    let result = ClayJsRuntimeService::default()
        .load_configuration_from_root(root)
        .await
        .unwrap();

    assert_eq!(
        result.op_records,
        vec!["project.md:degraded; visible syntax refresh delayed:degraded"]
    );
}

#[tokio::test]
async fn markdown_it_adapter_large_fixture_span_counts_are_stable() {
    let root = config_fixture("markdown-adapter-large-counts");
    let parser_source = fs::read_to_string("packages/markdown/dist/parser.js")
        .expect("markdown parser adapter must exist");
    fs::write(root.join("parser.js"), parser_source).unwrap();
    fs::write(
        root.join("init.js"),
        r##"
        import { parseMarkdownDecorations } from "./parser.js";

        const blockCount = 192;
        let text = "";
        const tokens = [];
        for (let index = 0; index < blockCount; index += 1) {
          const startLine = text.split("\n").length - 1;
          text += `# Heading ${index}\n\n`;
          text += `Paragraph ${index} has **strong**, *emphasis*, and \`code\`.\n\n`;
          text += "```js\nconst value = 1;\n```\n\n";
          text += `- bullet ${index}\n1. ordered ${index}\n\n`;
          tokens.push(
            { type: "heading_open", tag: "h1", map: [startLine, startLine + 1] },
            { type: "inline", map: [startLine, startLine + 1], content: `Heading ${index}`, children: [{ type: "text", content: `Heading ${index}` }] },
            { type: "heading_close" },
            { type: "paragraph_open", map: [startLine + 2, startLine + 3] },
            {
              type: "inline",
              map: [startLine + 2, startLine + 3],
              content: `Paragraph ${index} has **strong**, *emphasis*, and \`code\`.`,
              children: [
                { type: "text", content: `Paragraph ${index} has ` },
                { type: "strong_open", markup: "**" },
                { type: "text", content: "strong" },
                { type: "strong_close", markup: "**" },
                { type: "text", content: ", " },
                { type: "em_open", markup: "*" },
                { type: "text", content: "emphasis" },
                { type: "em_close", markup: "*" },
                { type: "text", content: ", and " },
                { type: "code_inline", markup: "`", content: "code" },
                { type: "text", content: "." }
              ]
            },
            { type: "paragraph_close" },
            { type: "fence", tag: "code", map: [startLine + 4, startLine + 7], markup: "```", info: "js" },
            { type: "bullet_list_open", map: [startLine + 8, startLine + 9] },
            { type: "list_item_open", map: [startLine + 8, startLine + 9] },
            { type: "paragraph_open", map: [startLine + 8, startLine + 9] },
            { type: "inline", map: [startLine + 8, startLine + 9], content: `bullet ${index}`, children: [{ type: "text", content: `bullet ${index}` }] },
            { type: "paragraph_close" },
            { type: "list_item_close" },
            { type: "bullet_list_close" },
            { type: "ordered_list_open", map: [startLine + 9, startLine + 10] },
            { type: "list_item_open", map: [startLine + 9, startLine + 10] },
            { type: "paragraph_open", map: [startLine + 9, startLine + 10] },
            { type: "inline", map: [startLine + 9, startLine + 10], content: `ordered ${index}`, children: [{ type: "text", content: `ordered ${index}` }] },
            { type: "paragraph_close" },
            { type: "list_item_close" },
            { type: "ordered_list_close" }
          );
        }
        function utf8ByteLength(value) {
          let bytes = 0;
          for (const character of value) {
            const codePoint = character.codePointAt(0);
            bytes += codePoint <= 0x7f ? 1 : codePoint <= 0x7ff ? 2 : codePoint <= 0xffff ? 3 : 4;
          }
          return bytes;
        }
        const viewport = { byteStart: 0, byteEnd: utf8ByteLength(text) };
        const first = await parseMarkdownDecorations({ text, tokens, viewport });
        const second = await parseMarkdownDecorations({ text, tokens, viewport });
        if (first.length !== second.length) throw new Error(`unstable span counts: ${first.length} != ${second.length}`);
        if (first.length !== blockCount * 7) throw new Error(`expected ${blockCount * 7} spans, got ${first.length}`);
        const byToken = new Map();
        function tokenKey(span) {
          return `${span.tokenType}:${(span.modifiers ?? []).join(",")}`;
        }
        for (const span of first) {
          const token = tokenKey(span);
          byToken.set(token, (byToken.get(token) ?? 0) + 1);
        }
        for (const [token, expected] of [
          ["Heading1:", blockCount],
          ["Paragraph:Bold", blockCount],
          ["Paragraph:Italic", blockCount],
          ["CodeSpan:", blockCount],
          ["CodeBlock:", blockCount],
          ["ListItem:", blockCount * 2],
        ]) {
          if (byToken.get(token) !== expected) throw new Error(`${token} expected ${expected}, got ${byToken.get(token)}`);
        }
        Deno.core.ops.op_clay_runtime_record(`${first.length}:${byToken.get("ListItem:")}`);
        "##,
    )
    .unwrap();

    let result = ClayJsRuntimeService::default()
        .load_configuration_from_root(root)
        .await
        .unwrap();

    assert_eq!(result.op_records, vec!["1344:384"]);
}

// Plan 061 task 15: trusted init.js configuration APIs for third-party
// package loading, replacement, and adoption-state diagnostics.
