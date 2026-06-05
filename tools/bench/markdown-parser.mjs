#!/usr/bin/env node
import fs from 'node:fs/promises';
import path from 'node:path';
import { performance } from 'node:perf_hooks';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { createRequire } from 'node:module';

const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(SCRIPT_DIR, '..', '..');
const PACKAGE_ROOT = path.join(REPO_ROOT, 'packages', 'markdown');
const packageRequire = createRequire(path.join(PACKAGE_ROOT, 'package.json'));
const DEFAULT_SIZES = ['64KiB', '256KiB', '1MiB', '5MiB', '16MiB'];
const DEFAULT_PARSERS = ['markdown-it', 'adapter', 'windowed-adapter'];
const EXCLUDED_DIRS = new Set(['.git', 'target', 'node_modules']);
const LARGE_FILE_THRESHOLD_BYTES = 5 * 1024 * 1024;
const SMALL_FILE_THRESHOLD_BYTES = 1 * 1024 * 1024;
const WINDOWED_PARSE_BYTES = 64 * 1024;
const WINDOWED_VIEWPORT_BYTES = 16 * 1024;
const MARKDOWN_OVERHEAD_BUDGET_BYTES = 30 * 1024 * 1024;

function parseArgs(argv) {
  const options = { sizes: DEFAULT_SIZES, parsers: DEFAULT_PARSERS, iterations: 3, warmup: 1, sourceLimit: 32, dryRun: false, json: false };
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    const next = () => {
      index += 1;
      if (index >= argv.length) throw new Error(`missing value for ${arg}`);
      return argv[index];
    };
    if (arg === '--sizes') options.sizes = next().split(',').filter(Boolean);
    else if (arg === '--parser' || arg === '--parsers') options.parsers = next().split(',').filter(Boolean);
    else if (arg === '--iterations') options.iterations = Number(next());
    else if (arg === '--warmup') options.warmup = Number(next());
    else if (arg === '--source-limit') options.sourceLimit = Number(next());
    else if (arg === '--dry-run') options.dryRun = true;
    else if (arg === '--json') options.json = true;
    else if (arg === '--help' || arg === '-h') { printHelp(); process.exit(0); }
    else throw new Error(`unknown argument: ${arg}`);
  }
  if (!Number.isInteger(options.iterations) || options.iterations < 1) throw new Error('--iterations must be a positive integer');
  if (!Number.isInteger(options.warmup) || options.warmup < 0) throw new Error('--warmup must be a non-negative integer');
  if (!Number.isInteger(options.sourceLimit) || options.sourceLimit < 1) throw new Error('--source-limit must be a positive integer');
  return options;
}

function printHelp() {
  console.log(`Usage: node --expose-gc tools/bench/markdown-parser.mjs [options]\n\nBenchmarks active Markdown parser paths on large corpora synthesized by repeating the largest Markdown files already committed in this repository. It does not create or mutate source fixtures during timing runs. Full-document adapter results are advisory evidence only for large files; windowed-adapter is the ordinary large-file editor path.\n\nOptions:\n  --sizes 64KiB,256KiB,1MiB,5MiB,16MiB\n                                 Comma-separated corpus sizes (default: ${DEFAULT_SIZES.join(',')})\n  --parser markdown-it,adapter,windowed-adapter\n                                 Parser set (default: ${DEFAULT_PARSERS.join(',')})\n  --iterations 3                 Timed iterations per parser/size\n  --warmup 1                     Untimed warmup iterations per parser/size\n  --source-limit 32              Number of largest repo .md files to seed corpora\n  --dry-run                      Build corpora and print coverage without importing parsers\n  --json                         Emit sanitized JSON instead of text\n`);
}

function parseSize(size) {
  const match = /^(\d+)(MiB|KiB|B)?$/i.exec(size.trim());
  if (!match) throw new Error(`invalid size: ${size}`);
  const value = Number(match[1]);
  const unit = (match[2] ?? 'B').toLowerCase();
  if (unit === 'mib') return value * 1024 * 1024;
  if (unit === 'kib') return value * 1024;
  return value;
}

function formatMiB(bytes) { return `${(bytes / (1024 * 1024)).toFixed(2)} MiB`; }
function toMiB(bytes) { return Number((bytes / (1024 * 1024)).toFixed(2)); }
function relativePath(filePath) { return path.relative(REPO_ROOT, filePath).split(path.sep).join('/'); }
function utf8Bytes(text) { return Buffer.byteLength(text, 'utf8'); }
function jsonBytes(value) { return Buffer.byteLength(JSON.stringify(value), 'utf8'); }

function utf8ByteLengthForCodePoint(codePoint) {
  if (codePoint <= 0x7f) return 1;
  if (codePoint <= 0x7ff) return 2;
  if (codePoint <= 0xffff) return 3;
  return 4;
}

function sliceUtf8ByByteLimit(text, limitBytes) {
  let bytes = 0;
  let end = 0;
  while (end < text.length) {
    const codePoint = text.codePointAt(end);
    const width = codePoint > 0xffff ? 2 : 1;
    const nextBytes = utf8ByteLengthForCodePoint(codePoint);
    if (bytes + nextBytes > limitBytes) break;
    bytes += nextBytes;
    end += width;
  }
  return { text: text.slice(0, end), bytes };
}

function buildParseWindow(text, requestedBytes = WINDOWED_PARSE_BYTES) {
  const window = sliceUtf8ByByteLimit(text, Math.min(requestedBytes, utf8Bytes(text)));
  return {
    byteStart: 0,
    byteEnd: window.bytes,
    baseLine: 0,
    text: window.text,
    viewport: { byteStart: 0, byteEnd: Math.min(WINDOWED_VIEWPORT_BYTES, window.bytes) }
  };
}

async function collectMarkdownFiles(dir = REPO_ROOT) {
  const entries = await fs.readdir(dir, { withFileTypes: true });
  const files = [];
  for (const entry of entries) {
    if (entry.isDirectory()) {
      if (EXCLUDED_DIRS.has(entry.name)) continue;
      files.push(...await collectMarkdownFiles(path.join(dir, entry.name)));
    } else if (entry.isFile() && entry.name.toLowerCase().endsWith('.md')) {
      const filePath = path.join(dir, entry.name);
      const stat = await fs.stat(filePath);
      files.push({ path: filePath, relativePath: relativePath(filePath), bytes: stat.size });
    }
  }
  return files.sort((left, right) => right.bytes - left.bytes || left.relativePath.localeCompare(right.relativePath));
}

async function loadSourceTexts(sourceLimit) {
  const files = (await collectMarkdownFiles()).slice(0, sourceLimit);
  if (files.length === 0) throw new Error('no Markdown files found in repository');
  const sources = [];
  for (const file of files) sources.push({ ...file, text: await fs.readFile(file.path, 'utf8') });
  return sources;
}

function buildCorpus(sources, targetBytes) {
  let text = '';
  let sourceIndex = 0;
  while (utf8Bytes(text) < targetBytes) {
    const source = sources[sourceIndex % sources.length];
    text += text.length === 0 ? source.text : `\n\n${source.text}`;
    sourceIndex += 1;
  }
  const exact = sliceUtf8ByByteLimit(text, targetBytes);
  return { text: exact.text, bytes: exact.bytes, sourceCopies: sourceIndex, window: buildParseWindow(exact.text) };
}

function syntaxCoverage(text) {
  return {
    headings: (text.match(/^#{1,6}(?:\s|$)/gm) ?? []).length,
    strong: (text.match(/\*\*[^*]+\*\*/g) ?? []).length,
    emphasis: (text.match(/(^|[^*])\*[^*\n]+\*/g) ?? []).length + (text.match(/(^|[^_])_[^_\n]+_/g) ?? []).length,
    inlineCode: (text.match(/`[^`\n]+`/g) ?? []).length,
    fencedCodeBlocks: (text.match(/^\s*(```|~~~)/gm) ?? []).length,
    unorderedLists: (text.match(/^\s*[-+*]\s+/gm) ?? []).length,
    orderedLists: (text.match(/^\s*\d+[.)]\s+/gm) ?? []).length,
    utf8: /[^\x00-\x7f]/.test(text)
  };
}

async function importModuleFromPackage(packageName) {
  try {
    return await import(pathToFileURL(packageRequire.resolve(packageName)).href);
  } catch (error) {
    if (error.code === 'MODULE_NOT_FOUND' || error.message.includes('Cannot find package')) {
      throw new Error(`missing ${packageName}; run: npm install --prefix packages/markdown --no-save --no-package-lock --ignore-scripts markdown-it@^14.1.0`);
    }
    throw error;
  }
}

async function loadBenchmarks() {
  const markdownItModule = await importModuleFromPackage('markdown-it');
  const parserAdapter = await import(pathToFileURL(path.join(PACKAGE_ROOT, 'dist', 'parser.js')).href);
  const MarkdownIt = markdownItModule.default ?? markdownItModule;
  const markdownIt = new MarkdownIt({ html: false, linkify: false, typographer: false });
  if (typeof parserAdapter.parseMarkdownDecorations !== 'function') throw new Error('packages/markdown/dist/parser.js did not export parseMarkdownDecorations');
  if (typeof parserAdapter.parseMarkdownDecorationUpdate !== 'function') throw new Error('packages/markdown/dist/parser.js did not export parseMarkdownDecorationUpdate');
  return {
    markdownIt: {
      parser: 'markdown-it',
      category: 'parser_full_document_advisory',
      inputKind: 'full-document',
      hotPathPolicy: 'advisory only for files above 1 MiB; never ordinary large-file edit/open/scroll work',
      async run(corpus) {
        const tokens = markdownIt.parse(corpus.text, {});
        return { resultCount: tokens.length, retainedDecorationCacheBytes: 0, parserInputBytes: corpus.bytes };
      }
    },
    adapter: {
      parser: 'adapter',
      category: 'adapter_full_document_advisory',
      inputKind: 'full-document',
      hotPathPolicy: 'large-file full-document adapter is advisory only and not an ordinary hot path',
      async run(corpus) {
        const spans = await parserAdapter.parseMarkdownDecorations({ text: corpus.text, markdownIt, viewport: { byteStart: 0, byteEnd: corpus.bytes } });
        return { resultCount: spans.length, retainedDecorationCacheBytes: jsonBytes(spans), parserInputBytes: corpus.bytes };
      }
    },
    windowedAdapter: {
      parser: 'windowed-adapter',
      category: 'adapter_windowed_viewport',
      inputKind: 'bounded-parse-window',
      hotPathPolicy: 'ordinary medium/large-file visible decoration refresh path',
      async run(corpus) {
        const parseWindow = corpus.window;
        const spans = await parserAdapter.parseMarkdownDecorations({
          markdownIt,
          parseWindows: [parseWindow],
          viewport: parseWindow.viewport,
          memoryBudgetBytes: MARKDOWN_OVERHEAD_BUDGET_BYTES
        });
        return { resultCount: spans.length, retainedDecorationCacheBytes: jsonBytes(spans), parserInputBytes: parseWindow.text.length === corpus.text.length ? corpus.bytes : parseWindow.byteEnd - parseWindow.byteStart };
      }
    },
    statusFallback: {
      parser: 'status-fallback',
      category: 'status_fallback_policy',
      inputKind: 'bounded-parse-window-status',
      hotPathPolicy: 'load/reload/explicit viewport status path; never paint or keypress work',
      async run(corpus) {
        const parseWindow = corpus.window;
        const update = await parserAdapter.parseMarkdownDecorationUpdate({
          documentId: 7,
          documentVersion: 3,
          behaviorVersion: 3,
          packagePrefix: 'markdown',
          parseWindows: [parseWindow],
          viewport: parseWindow.viewport,
          budgetExceeded: true,
          memoryBudgetBytes: MARKDOWN_OVERHEAD_BUDGET_BYTES
        });
        return { resultCount: update.status?.highlightingState === 'plain-text-fallback' ? 1 : 0, retainedDecorationCacheBytes: jsonBytes(update.status ?? {}), parserInputBytes: 0 };
      }
    }
  };
}

function percentile(values, quantile) {
  const sorted = [...values].sort((a, b) => a - b);
  const index = Math.min(sorted.length - 1, Math.ceil(sorted.length * quantile) - 1);
  return sorted[index];
}

function normalizeRunResult(result) {
  if (typeof result === 'number') {
    return { resultCount: result, retainedDecorationCacheBytes: 0, parserInputBytes: 0 };
  }
  return {
    resultCount: Number(result?.resultCount ?? 0),
    retainedDecorationCacheBytes: Number(result?.retainedDecorationCacheBytes ?? 0),
    parserInputBytes: Number(result?.parserInputBytes ?? 0)
  };
}

async function measure({ benchmark, corpus, iterations, warmup, baselineRss }) {
  for (let index = 0; index < warmup; index += 1) await benchmark.run(corpus);
  globalThis.gc?.();
  const before = process.memoryUsage();
  const samplesMs = [];
  const resultCounts = [];
  const retainedBytes = [];
  const inputBytes = [];
  let peakRss = before.rss;
  let peakHeapUsed = before.heapUsed;
  for (let index = 0; index < iterations; index += 1) {
    const started = performance.now();
    const result = normalizeRunResult(await benchmark.run(corpus));
    samplesMs.push(performance.now() - started);
    resultCounts.push(result.resultCount);
    retainedBytes.push(result.retainedDecorationCacheBytes);
    inputBytes.push(result.parserInputBytes);
    const current = process.memoryUsage();
    peakRss = Math.max(peakRss, current.rss);
    peakHeapUsed = Math.max(peakHeapUsed, current.heapUsed);
    globalThis.gc?.();
  }
  const after = process.memoryUsage();
  const retainedDecorationCacheMemory = Math.max(0, ...retainedBytes);
  const markdownParserTemporaryAllocations = Math.max(0, peakHeapUsed - before.heapUsed);
  const markdownOverhead = markdownParserTemporaryAllocations + retainedDecorationCacheMemory;
  const hotPathAllowed = benchmark.parser === 'windowed-adapter' || corpus.bytes <= SMALL_FILE_THRESHOLD_BYTES;
  return {
    parser: benchmark.parser,
    category: benchmark.category,
    inputKind: benchmark.inputKind,
    hotPathAllowed,
    hotPathPolicy: benchmark.hotPathPolicy,
    parserInputBytes: Math.max(0, ...inputBytes),
    samplesMs: samplesMs.map((value) => Number(value.toFixed(3))),
    meanMs: Number((samplesMs.reduce((sum, value) => sum + value, 0) / samplesMs.length).toFixed(3)),
    p95Ms: Number(percentile(samplesMs, 0.95).toFixed(3)),
    resultCount: resultCounts[0],
    stableResultCount: resultCounts.every((value) => value === resultCounts[0]),
    memory: {
      total_rss: peakRss,
      total_rss_mib: toMiB(peakRss),
      baseline_rss: baselineRss,
      baseline_rss_mib: toMiB(baselineRss),
      document_memory: corpus.bytes,
      document_memory_mib: toMiB(corpus.bytes),
      markdown_parser_temporary_allocations: markdownParserTemporaryAllocations,
      markdown_parser_temporary_allocations_mib: toMiB(markdownParserTemporaryAllocations),
      retained_decoration_cache_memory: retainedDecorationCacheMemory,
      retained_decoration_cache_memory_mib: toMiB(retainedDecorationCacheMemory),
      markdown_overhead: markdownOverhead,
      markdown_overhead_mib: toMiB(markdownOverhead),
      markdown_overhead_budget: MARKDOWN_OVERHEAD_BUDGET_BYTES,
      markdown_overhead_budget_mib: toMiB(MARKDOWN_OVERHEAD_BUDGET_BYTES),
      markdown_overhead_budget_met: markdownOverhead <= MARKDOWN_OVERHEAD_BUDGET_BYTES,
      heap_delta_mib: toMiB(after.heapUsed - before.heapUsed),
      peak_heap_used_mib: toMiB(peakHeapUsed),
      rss_over_baseline_mib: toMiB(Math.max(0, peakRss - baselineRss))
    }
  };
}

async function main() {
  const options = parseArgs(process.argv.slice(2));
  globalThis.gc?.();
  const baselineRss = process.memoryUsage().rss;
  const sources = await loadSourceTexts(options.sourceLimit);
  const corpora = options.sizes.map((size) => ({ requestedSize: size, ...buildCorpus(sources, parseSize(size)) }));
  const benchmarks = options.dryRun ? null : await loadBenchmarks();
  const results = [];
  for (const corpus of corpora) {
    const fixture = {
      requestedSize: corpus.requestedSize,
      bytes: corpus.bytes,
      sizeMiB: toMiB(corpus.bytes),
      sourceCopies: corpus.sourceCopies,
      syntaxCoverage: syntaxCoverage(corpus.text),
      windowedViewport: {
        byteStart: corpus.window.viewport.byteStart,
        byteEnd: corpus.window.viewport.byteEnd,
        parseWindowBytes: corpus.window.byteEnd - corpus.window.byteStart
      },
      largeFilePolicy: corpus.bytes > LARGE_FILE_THRESHOLD_BYTES ? 'large-windowed-only' : (corpus.bytes > SMALL_FILE_THRESHOLD_BYTES ? 'medium-windowed-default' : 'small-full-document-allowed')
    };
    const parserResults = [];
    let statusFallbackResult = null;
    if (!options.dryRun) {
      for (const parser of options.parsers) {
        const normalized = parser === 'markdown-it' ? 'markdownIt' : parser === 'windowed-adapter' ? 'windowedAdapter' : parser;
        if (!benchmarks[normalized]) throw new Error(`unsupported parser: ${parser}`);
        parserResults.push(await measure({ benchmark: benchmarks[normalized], corpus, iterations: options.iterations, warmup: options.warmup, baselineRss }));
      }
      statusFallbackResult = await measure({ benchmark: benchmarks.statusFallback, corpus, iterations: options.iterations, warmup: options.warmup, baselineRss });
    }
    results.push({ fixture, parserResults, statusFallbackResult });
  }
  const output = {
    generatedAt: new Date().toISOString(),
    node: process.version,
    repoRoot: '<repo>',
    corpusPolicy: 'largest committed repository Markdown files repeated to requested sizes; no dummy prose generated and no source fixtures mutated',
    benchmarkPolicy: 'timings are local advisory evidence; hard gates are deterministic no-full-document-hot-path, payload, cache-budget, and benchmark compile checks',
    memoryAccounting: 'total_rss and baseline_rss are reported for triage; the 30 MiB pass/fail budget applies to markdown_overhead = markdown_parser_temporary_allocations + retained_decoration_cache_memory',
    sourceFiles: sources.map(({ relativePath, bytes }) => ({ relativePath, bytes })),
    options: { ...options, repoRoot: '<repo>' },
    results
  };
  if (options.json) console.log(JSON.stringify(output, null, 2)); else printReport(output);
}

function printReport(output) {
  console.log('Markdown parser benchmark');
  console.log(`Node: ${output.node}`);
  console.log(`Corpus: ${output.corpusPolicy}`);
  console.log(`Memory: ${output.memoryAccounting}`);
  console.log('Source files:');
  for (const source of output.sourceFiles.slice(0, 10)) console.log(`  - ${source.relativePath} (${formatMiB(source.bytes)})`);
  if (output.sourceFiles.length > 10) console.log(`  - ... ${output.sourceFiles.length - 10} more`);
  for (const result of output.results) {
    console.log(`\nFixture ${result.fixture.requestedSize}: ${formatMiB(result.fixture.bytes)} from ${result.fixture.sourceCopies} source copies (${result.fixture.largeFilePolicy})`);
    console.log(`  coverage: ${JSON.stringify(result.fixture.syntaxCoverage)}`);
    console.log(`  windowed viewport: ${result.fixture.windowedViewport.byteEnd - result.fixture.windowedViewport.byteStart} bytes visible, ${result.fixture.windowedViewport.parseWindowBytes} bytes parsed`);
    for (const parser of result.parserResults) {
      console.log(`  ${parser.parser}: category=${parser.category} hotPathAllowed=${parser.hotPathAllowed} input=${parser.parserInputBytes} bytes mean=${parser.meanMs} ms p95=${parser.p95Ms} ms samples=[${parser.samplesMs.join(', ')}] count=${parser.resultCount}${parser.stableResultCount ? '' : ' (unstable!)'} markdown_overhead=${parser.memory.markdown_overhead_mib} MiB budgetMet=${parser.memory.markdown_overhead_budget_met} total_rss=${parser.memory.total_rss_mib} MiB baseline_rss=${parser.memory.baseline_rss_mib} MiB`);
    }
    if (result.statusFallbackResult) {
      const status = result.statusFallbackResult;
      console.log(`  ${status.parser}: category=${status.category} hotPathAllowed=${status.hotPathAllowed} mean=${status.meanMs} ms count=${status.resultCount} markdown_overhead=${status.memory.markdown_overhead_mib} MiB`);
    }
  }
}

main().catch((error) => { console.error(error.message); process.exitCode = 1; });
