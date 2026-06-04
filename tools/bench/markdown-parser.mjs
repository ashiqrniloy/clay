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
const DEFAULT_SIZES = ['1MiB', '5MiB', '16MiB'];
const DEFAULT_PARSERS = ['markdown-it', 'adapter'];
const EXCLUDED_DIRS = new Set(['.git', 'target', 'node_modules']);

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
  console.log(`Usage: node --expose-gc tools/bench/markdown-parser.mjs [options]\n\nBenchmarks the active Markdown parser paths on large corpora synthesized by repeating the largest Markdown files already committed in this repository. It does not create or mutate source fixtures during timing runs.\n\nOptions:\n  --sizes 1MiB,5MiB,16MiB       Comma-separated corpus sizes (default: ${DEFAULT_SIZES.join(',')})\n  --parser markdown-it,adapter  Parser set: markdown-it, adapter (default: ${DEFAULT_PARSERS.join(',')})\n  --iterations 3                Timed iterations per parser/size\n  --warmup 1                    Untimed warmup iterations per parser/size\n  --source-limit 32             Number of largest repo .md files to seed corpora\n  --dry-run                     Build corpora and print coverage without importing parsers\n  --json                        Emit JSON instead of text\n`);
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
function relativePath(filePath) { return path.relative(REPO_ROOT, filePath).split(path.sep).join('/'); }

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
  while (Buffer.byteLength(text, 'utf8') < targetBytes) {
    const source = sources[sourceIndex % sources.length];
    text += text.length === 0 ? source.text : `\n\n${source.text}`;
    sourceIndex += 1;
  }
  return { text, bytes: Buffer.byteLength(text, 'utf8'), sourceCopies: sourceIndex };
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
  return {
    markdownIt(text) { return markdownIt.parse(text, {}).length; },
    async adapter(text) {
      const spans = await parserAdapter.parseMarkdownDecorations({ text, markdownIt, viewport: { byteStart: 0, byteEnd: Buffer.byteLength(text, 'utf8') } });
      return spans.length;
    }
  };
}

function percentile(values, quantile) {
  const sorted = [...values].sort((a, b) => a - b);
  const index = Math.min(sorted.length - 1, Math.ceil(sorted.length * quantile) - 1);
  return sorted[index];
}

async function measure({ name, run, text, iterations, warmup }) {
  for (let index = 0; index < warmup; index += 1) await run(text);
  globalThis.gc?.();
  const before = process.memoryUsage();
  const samplesMs = [];
  const resultCounts = [];
  let peakRss = before.rss;
  let peakHeapUsed = before.heapUsed;
  for (let index = 0; index < iterations; index += 1) {
    const started = performance.now();
    const count = await run(text);
    samplesMs.push(performance.now() - started);
    resultCounts.push(count);
    const current = process.memoryUsage();
    peakRss = Math.max(peakRss, current.rss);
    peakHeapUsed = Math.max(peakHeapUsed, current.heapUsed);
    globalThis.gc?.();
  }
  const after = process.memoryUsage();
  return {
    parser: name,
    samplesMs: samplesMs.map((value) => Number(value.toFixed(3))),
    meanMs: Number((samplesMs.reduce((sum, value) => sum + value, 0) / samplesMs.length).toFixed(3)),
    p95Ms: Number(percentile(samplesMs, 0.95).toFixed(3)),
    resultCount: resultCounts[0],
    stableResultCount: resultCounts.every((value) => value === resultCounts[0]),
    heapDeltaMiB: Number(((after.heapUsed - before.heapUsed) / (1024 * 1024)).toFixed(2)),
    peakHeapUsedMiB: Number((peakHeapUsed / (1024 * 1024)).toFixed(2)),
    peakRssMiB: Number((peakRss / (1024 * 1024)).toFixed(2))
  };
}

async function main() {
  const options = parseArgs(process.argv.slice(2));
  const sources = await loadSourceTexts(options.sourceLimit);
  const corpora = options.sizes.map((size) => ({ requestedSize: size, ...buildCorpus(sources, parseSize(size)) }));
  const benchmarks = options.dryRun ? null : await loadBenchmarks();
  const results = [];
  for (const corpus of corpora) {
    const fixture = { requestedSize: corpus.requestedSize, bytes: corpus.bytes, sizeMiB: Number((corpus.bytes / (1024 * 1024)).toFixed(2)), sourceCopies: corpus.sourceCopies, syntaxCoverage: syntaxCoverage(corpus.text) };
    const parserResults = [];
    if (!options.dryRun) {
      for (const parser of options.parsers) {
        const normalized = parser === 'markdown-it' ? 'markdownIt' : parser;
        if (!benchmarks[normalized]) throw new Error(`unsupported parser: ${parser}`);
        parserResults.push(await measure({ name: parser, run: benchmarks[normalized], text: corpus.text, iterations: options.iterations, warmup: options.warmup }));
      }
    }
    results.push({ fixture, parserResults });
  }
  const output = {
    generatedAt: new Date().toISOString(),
    node: process.version,
    repoRoot: '<repo>',
    corpusPolicy: 'largest committed repository Markdown files repeated to requested sizes; no dummy prose generated and no source fixtures mutated',
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
  console.log('Source files:');
  for (const source of output.sourceFiles.slice(0, 10)) console.log(`  - ${source.relativePath} (${formatMiB(source.bytes)})`);
  if (output.sourceFiles.length > 10) console.log(`  - ... ${output.sourceFiles.length - 10} more`);
  for (const result of output.results) {
    console.log(`\nFixture ${result.fixture.requestedSize}: ${formatMiB(result.fixture.bytes)} from ${result.fixture.sourceCopies} source copies`);
    console.log(`  coverage: ${JSON.stringify(result.fixture.syntaxCoverage)}`);
    for (const parser of result.parserResults) {
      console.log(`  ${parser.parser}: mean=${parser.meanMs} ms p95=${parser.p95Ms} ms samples=[${parser.samplesMs.join(', ')}] count=${parser.resultCount}${parser.stableResultCount ? '' : ' (unstable!)'} heapDelta=${parser.heapDeltaMiB} MiB peakHeap=${parser.peakHeapUsedMiB} MiB peakRss=${parser.peakRssMiB} MiB`);
    }
  }
}

main().catch((error) => { console.error(error.message); process.exitCode = 1; });
