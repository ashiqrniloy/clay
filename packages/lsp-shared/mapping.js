import { utf8ByteLength } from "./utf8.js";

const MAX_SEMANTIC_TOKENS = 128;
const MAX_DIAGNOSTICS = 128;
const MAX_COMPLETIONS = 256;
const MAX_DEFINITIONS = 64;
const MAX_CODE_ACTIONS = 64;
const MAX_SIGNATURES = 16;
const MAX_PARAMETERS = 32;
const MAX_MARKDOWN_CHARS = 4096;
const MAX_FIELD_CHARS = 1024;
const DECORATION_PAYLOAD_BYTES = 8 * 1024;
const DIAGNOSTIC_PAYLOAD_BYTES = 8 * 1024;
const RESULT_PAYLOAD_BYTES = 16 * 1024;
const TOKEN_TYPES = new Map([
  ["namespace", "Namespace"], ["type", "Type"], ["class", "Class"], ["enum", "Enum"],
  ["interface", "Interface"], ["struct", "Struct"], ["typeParameter", "TypeParameter"],
  ["parameter", "Parameter"], ["variable", "Variable"], ["property", "Property"],
  ["enumMember", "EnumMember"], ["event", "Event"], ["function", "Function"],
  ["method", "Method"], ["member", "Method"], ["macro", "Macro"], ["keyword", "Keyword"],
  ["modifier", "Modifier"], ["comment", "Comment"], ["string", "String"], ["number", "Number"],
  ["regexp", "Regexp"], ["operator", "Operator"], ["decorator", "Decorator"], ["label", "Variable"],
]);
const TOKEN_MODIFIERS = new Map([
  ["declaration", "Declaration"], ["definition", "Definition"], ["readonly", "Readonly"],
  ["static", "Static"], ["deprecated", "Deprecated"], ["unused", "Deprecated"], ["unnecessary", "Deprecated"], ["abstract", "Abstract"],
  ["async", "Async"], ["modification", "Modification"], ["documentation", "Documentation"],
  ["defaultLibrary", "DefaultLibrary"],
]);

function boundedString(value, limit = MAX_FIELD_CHARS) {
  return typeof value === "string" ? value.slice(0, limit) : "";
}

function boundedPayload(value, limit, kind) {
  if (utf8ByteLength(JSON.stringify(value)) > limit) {
    throw new Error(`lsp.${kind}_too_large: mapped payload exceeds Clay budget`);
  }
  return value;
}

function markdown(value) {
  let text = "";
  if (typeof value === "string") text = value;
  else if (value && typeof value === "object" && typeof value.language === "string") text = `\`\`\`${value.language}\n${boundedString(value.value, MAX_MARKDOWN_CHARS)}\n\`\`\``;
  else if (value && typeof value === "object" && typeof value.value === "string") text = value.value;
  else if (Array.isArray(value)) text = value.map((entry) => markdown(entry)).join("\n\n");
  return text.replace(/<script\b[^>]*>[\s\S]*?<\/script\s*>/gi, "")
    .replace(/<[^>]+>/g, "")
    .slice(0, MAX_MARKDOWN_CHARS);
}

function providerEnabled(capability) {
  if (capability === undefined || capability === null || capability === false) return false;
  if (capability === true || (typeof capability === "object" && !Array.isArray(capability))) return true;
  throw new Error("lsp.invalid_capabilities: provider capability must be boolean or object");
}

function triggerCharacters(value) {
  const values = value ?? [];
  if (!Array.isArray(values) || values.length > 32 || values.some((item) => typeof item !== "string" || item.length === 0 || item.length > 8)) {
    throw new Error("lsp.invalid_capabilities: trigger characters are malformed or oversized");
  }
  return [...values];
}

export function parseCapabilities(initializeResult) {
  const capabilities = initializeResult?.capabilities;
  if (!capabilities || typeof capabilities !== "object" || Array.isArray(capabilities)) throw new Error("lsp.invalid_capabilities: initialize result lacks capabilities");
  const encoding = capabilities.positionEncoding ?? "utf-16";
  if (!["utf-8", "utf-16", "utf-32"].includes(encoding)) throw new Error("lsp.invalid_capabilities: unsupported position encoding");
  const numericSync = typeof capabilities.textDocumentSync === "number";
  if (!numericSync && capabilities.textDocumentSync !== undefined
      && (capabilities.textDocumentSync === null || typeof capabilities.textDocumentSync !== "object"
        || Array.isArray(capabilities.textDocumentSync))) {
    throw new Error("lsp.invalid_capabilities: text sync must be a kind or options object");
  }
  const sync = numericSync ? capabilities.textDocumentSync : capabilities.textDocumentSync?.change ?? 0;
  const openClose = numericSync ? sync !== 0 : capabilities.textDocumentSync?.openClose === true;
  if (![0, 1, 2].includes(sync)) throw new Error("lsp.invalid_capabilities: unsupported text sync kind");
  const semantic = capabilities.semanticTokensProvider;
  const legend = semantic?.legend ?? { tokenTypes: [], tokenModifiers: [] };
  if (!Array.isArray(legend.tokenTypes) || !Array.isArray(legend.tokenModifiers)
      || legend.tokenTypes.length > 256 || legend.tokenModifiers.length > 32) {
    throw new Error("lsp.invalid_capabilities: semantic token legend is malformed or oversized");
  }
  return Object.freeze({
    positionEncoding: encoding,
    textDocumentSync: sync,
    textDocumentOpenClose: openClose,
    completion: providerEnabled(capabilities.completionProvider),
    completionTriggerCharacters: triggerCharacters(capabilities.completionProvider?.triggerCharacters),
    hover: providerEnabled(capabilities.hoverProvider),
    definition: providerEnabled(capabilities.definitionProvider),
    codeAction: providerEnabled(capabilities.codeActionProvider),
    signatureHelp: providerEnabled(capabilities.signatureHelpProvider),
    signatureTriggerCharacters: triggerCharacters(capabilities.signatureHelpProvider?.triggerCharacters),
    signatureRetriggerCharacters: triggerCharacters(capabilities.signatureHelpProvider?.retriggerCharacters),
    semanticTokens: providerEnabled(semantic),
    pullDiagnostics: providerEnabled(capabilities.diagnosticProvider),
    semanticTokensFull: semantic?.full ?? false,
    semanticTokensRange: semantic?.range ?? false,
    inlayHint: providerEnabled(capabilities.inlayHintProvider),
    semanticLegend: {
      tokenTypes: legend.tokenTypes.slice(0, 256).map((item) => boundedString(item, 64)),
      tokenModifiers: legend.tokenModifiers.slice(0, 32).map((item) => boundedString(item, 64)),
    },
  });
}

export function applySemanticTokenDelta(previous, edits) {
  if (!Array.isArray(previous) || !Array.isArray(edits)) throw new Error("lsp.invalid_semantic_delta: arrays required");
  const result = previous.slice();
  let shift = 0;
  for (const edit of edits) {
    const start = edit?.start;
    const deleteCount = edit?.deleteCount;
    const data = edit?.data ?? [];
    if (!Number.isInteger(start) || !Number.isInteger(deleteCount) || start < 0 || deleteCount < 0 || !Array.isArray(data)) {
      throw new Error("lsp.invalid_semantic_delta: malformed edit");
    }
    const index = start + shift;
    if (index < 0 || index + deleteCount > result.length || data.some((item) => !Number.isInteger(item) || item < 0)) {
      throw new Error("lsp.invalid_semantic_delta: edit exceeds token data");
    }
    result.splice(index, deleteCount, ...data);
    shift += data.length - deleteCount;
    if (result.length > MAX_SEMANTIC_TOKENS * 5) throw new Error("lsp.semantic_tokens_too_large: token data exceeds budget");
  }
  return result;
}

export function semanticTokensToClay(data, legend, document) {
  if (!Array.isArray(data) || data.length % 5 !== 0 || data.length > MAX_SEMANTIC_TOKENS * 5) {
    throw new Error("lsp.invalid_semantic_tokens: bounded five-integer records required");
  }
  const spans = [];
  let line = 0;
  let character = 0;
  for (let index = 0; index < data.length; index += 5) {
    const [deltaLine, deltaStart, length, tokenType, modifierBits] = data.slice(index, index + 5);
    if (![deltaLine, deltaStart, length, tokenType, modifierBits].every((item) => Number.isInteger(item) && item >= 0)
        || length === 0 || modifierBits >= 2 ** legend.tokenModifiers.length) {
      throw new Error("lsp.invalid_semantic_tokens: token fields or modifier bits are invalid");
    }
    line += deltaLine;
    character = deltaLine === 0 ? character + deltaStart : deltaStart;
    const byteStart = document.positionToByte({ line, character });
    const byteEnd = document.positionToByte({ line, character: character + length });
    const tokenName = legend.tokenTypes[tokenType];
    if (typeof tokenName !== "string") throw new Error("lsp.invalid_semantic_tokens: token type exceeds legend");
    const modifiers = [];
    for (let bit = 0; bit < legend.tokenModifiers.length; bit += 1) {
      if (Math.floor(modifierBits / 2 ** bit) % 2 === 1) {
        const modifier = TOKEN_MODIFIERS.get(legend.tokenModifiers[bit]);
        if (modifier) modifiers.push(modifier);
      }
    }
    spans.push({
      byteStart,
      byteEnd,
      kind: "semantic",
      tokenType: TOKEN_TYPES.get(tokenName) ?? "Variable",
      modifiers,
      priority: 100,
    });
  }
  return boundedPayload(spans, DECORATION_PAYLOAD_BYTES, "semantic_tokens");
}

export function diagnosticsToClay(diagnostics, document) {
  if (!Array.isArray(diagnostics) || diagnostics.length > MAX_DIAGNOSTICS) throw new Error("lsp.diagnostics_too_large: diagnostics exceed budget");
  return boundedPayload(diagnostics.map((item) => ({
    ...document.rangeToBytes(item.range),
    severity: ({ 1: "error", 2: "warning", 3: "info", 4: "info" })[item.severity] ?? "info",
    code: boundedString(typeof item.code === "object" ? item.code?.value : String(item.code ?? ""), 128),
    message: boundedString(item.message, MAX_MARKDOWN_CHARS),
    source: boundedString(item.source ?? "lsp", 128),
  })), DIAGNOSTIC_PAYLOAD_BYTES, "diagnostics");
}

export function completionToClay(result) {
  const items = Array.isArray(result) ? result : result?.items ?? [];
  if (!Array.isArray(items) || items.length > MAX_COMPLETIONS) throw new Error("lsp.completions_too_large: completion items exceed budget");
  return boundedPayload({
    status: items.length === 0 ? "empty" : "ok",
    items: items.map((item) => {
      if (item?.additionalTextEdits || item?.command) throw new Error("lsp.mutating_completion_rejected: completion carries edits or command");
      const label = typeof item?.label === "string" ? item.label : item?.label?.label;
      if (typeof label !== "string" || label.length === 0) throw new Error("lsp.invalid_completion: label required");
      const insertText = item.textEdit?.newText ?? item.insertText ?? label;
      return {
        label: boundedString(label),
        insertText: boundedString(insertText, MAX_MARKDOWN_CHARS),
        detail: boundedString(item.detail),
        commitCharacters: Array.isArray(item.commitCharacters) ? item.commitCharacters.join("").slice(0, 64) : "",
        textFormat: item.insertTextFormat === 2 ? "snippet" : "plainText",
      };
    }),
  }, RESULT_PAYLOAD_BYTES, "completions");
}

const MAX_INLAY_HINTS = 64;
const MAX_INLAY_LABEL_CHARS = 64;

function inlayLabelText(label) {
  if (typeof label === "string") return label;
  if (!Array.isArray(label)) return "";
  return label.map((part) => (typeof part === "string" ? part : String(part?.value ?? ""))).join("");
}

function sanitizeInlayLabel(text) {
  return [...String(text)]
    .filter((ch) => ch.charCodeAt(0) >= 32)
    .slice(0, MAX_INLAY_LABEL_CHARS)
    .join("")
    .trim();
}

export function inlayHintsToClay(hints, document) {
  if (hints == null) return [];
  if (!Array.isArray(hints) || hints.length > MAX_INLAY_HINTS) {
    throw new Error("lsp.inlays_too_large: inlay hints exceed budget");
  }
  const spans = [];
  for (const hint of hints) {
    const label = sanitizeInlayLabel(inlayLabelText(hint?.label));
    if (!label || !hint?.position) continue;
    const offset = document.positionToByte(hint.position);
    const end = document.bytes.length;
    if (end === 0) continue;
    const byteStart = offset < end ? offset : end - 1;
    const byteEnd = byteStart + 1;
    spans.push({
      byteStart,
      byteEnd,
      kind: "inlayHint",
      tokenType: hint.kind === 2 ? "Parameter" : "Type",
      modifiers: [],
      priority: 10,
      inlay: { label, placement: hint.kind === 2 ? "before" : "after" },
    });
  }
  return boundedPayload(spans, DECORATION_PAYLOAD_BYTES, "inlay_hints");
}

export function hoverToClay(result, document) {
  if (!result) return { status: "empty", hover: { markdown: "" } };
  return boundedPayload({
    status: "ok",
    hover: {
      markdown: markdown(result.contents),
      ...(result.range ? { range: document.rangeToBytes(result.range) } : {}),
    },
  }, RESULT_PAYLOAD_BYTES, "hover");
}

export function definitionToClay(result, resolveLocation) {
  const values = result === null ? [] : Array.isArray(result) ? result : [result];
  if (values.length > MAX_DEFINITIONS) throw new Error("lsp.definitions_too_large: locations exceed budget");
  const locations = values.map((item) => resolveLocation({
    uri: item.targetUri ?? item.uri,
    range: item.targetSelectionRange ?? item.targetRange ?? item.range,
  }));
  return boundedPayload(
    { status: locations.length === 0 ? "empty" : "ok", definition: { locations } },
    RESULT_PAYLOAD_BYTES,
    "definitions",
  );
}

export function codeActionsToClay(result, commandAllowlist = new Map()) {
  if (result === null) return { status: "empty", codeAction: { actions: [] } };
  if (!Array.isArray(result) || result.length > MAX_CODE_ACTIONS) throw new Error("lsp.code_actions_too_large: actions exceed budget");
  const actions = [];
  for (const item of result) {
    if (!item || typeof item.title !== "string" || item.disabled || item.edit) continue;
    const command = item.command?.command ?? (typeof item.command === "string" ? item.command : undefined);
    if (command && !commandAllowlist.has(command)) continue;
    actions.push({
      title: boundedString(item.title),
      ...(command ? { commandId: commandAllowlist.get(command) } : {}),
    });
  }
  return boundedPayload(
    { status: actions.length === 0 ? "empty" : "ok", codeAction: { actions } },
    RESULT_PAYLOAD_BYTES,
    "code_actions",
  );
}

export function signatureHelpToClay(result) {
  if (!result) return { status: "empty", signatureHelp: { signatures: [] } };
  const signatures = result.signatures ?? [];
  if (!Array.isArray(signatures) || signatures.length > MAX_SIGNATURES) throw new Error("lsp.signatures_too_large: signatures exceed budget");
  if (result.activeSignature !== undefined
      && (!Number.isInteger(result.activeSignature) || result.activeSignature < 0 || result.activeSignature >= signatures.length)) {
    throw new Error("lsp.invalid_signature_help: active signature is outside result");
  }
  const active = signatures[result.activeSignature ?? 0];
  if (result.activeParameter !== undefined
      && (!Number.isInteger(result.activeParameter) || result.activeParameter < 0
        || !active || result.activeParameter >= (active.parameters?.length ?? 0))) {
    throw new Error("lsp.invalid_signature_help: active parameter is outside signature");
  }
  return boundedPayload({
    status: signatures.length === 0 ? "empty" : "ok",
    signatureHelp: {
      signatures: signatures.map((signature) => {
        const label = boundedString(signature.label);
        const parameters = signature.parameters ?? [];
        if (!Array.isArray(parameters) || parameters.length > MAX_PARAMETERS) throw new Error("lsp.parameters_too_large: parameters exceed budget");
        return {
          label,
          documentation: markdown(signature.documentation),
          parameters: parameters.map((parameter) => {
            let parameterLabel = parameter.label;
            if (Array.isArray(parameterLabel)) {
              const [start, end] = parameterLabel;
              if (!Number.isInteger(start) || !Number.isInteger(end) || start < 0 || start > end || end > label.length) {
                throw new Error("lsp.invalid_signature_help: parameter label range is invalid");
              }
              parameterLabel = label.slice(start, end);
            }
            return {
              label: boundedString(parameterLabel),
              documentation: markdown(parameter.documentation),
            };
          }),
        };
      }),
      activeSignature: result.activeSignature,
      activeParameter: result.activeParameter,
    },
  }, RESULT_PAYLOAD_BYTES, "signatures");
}
