/** Capability/response profiles for the generic Clay LSP fake server. */

function range(start, end, line = 0) {
  return {
    start: { line, character: start },
    end: { line, character: end },
  };
}

function baseInitialize(id, capabilities) {
  return {
    jsonrpc: "2.0",
    id,
    result: {
      capabilities,
      serverInfo: { name: "clay-fake-lsp", version: "1.0.0" },
    },
  };
}

export const PROFILES = {
  rust: {
    languageId: "rust",
    relativePath: "src/main.rs",
    uri: "file:///workspace/src/main.rs",
    text: "fn main() {}\n",
    capabilities: {
      positionEncoding: "utf-8",
      textDocumentSync: { openClose: true, change: 2 },
      completionProvider: { triggerCharacters: [":", ".", "'", "("] },
      hoverProvider: true,
      definitionProvider: true,
      codeActionProvider: true,
      signatureHelpProvider: { triggerCharacters: ["(", ",", "<"] },
      semanticTokensProvider: {
        full: { delta: true },
        legend: { tokenTypes: ["function"], tokenModifiers: ["declaration"] },
      },
      diagnosticProvider: { identifier: "rust-analyzer" },
    },
    respond(message, state) {
      if (message.method === "initialize") {
        return [baseInitialize(message.id, this.capabilities)];
      }
      if (message.method === "textDocument/semanticTokens/full") {
        state.semanticRequests = (state.semanticRequests ?? 0) + 1;
        state.uri = message.params?.textDocument?.uri ?? state.uri ?? this.uri;
        return [
          {
            jsonrpc: "2.0",
            method: "textDocument/publishDiagnostics",
            params: {
              uri: state.uri,
              version: 0,
              diagnostics: [{ range: range(0, 2), severity: 1, message: "stale" }],
            },
          },
          {
            jsonrpc: "2.0",
            id: message.id,
            result: { resultId: "semantic-1", data: [0, 0, 2, 0, 1] },
          },
        ];
      }
      if (message.method === "textDocument/semanticTokens/full/delta") {
        state.semanticRequests = (state.semanticRequests ?? 0) + 1;
        return [{
          jsonrpc: "2.0",
          id: message.id,
          result: {
            resultId: "semantic-2",
            edits: [{ start: 0, deleteCount: 5, data: [0, 0, 2, 0, 1] }],
          },
        }];
      }
      if (message.method === "textDocument/diagnostic") {
        return [{
          jsonrpc: "2.0",
          id: message.id,
          result: message.params?.previousResultId
            ? { kind: "unchanged", resultId: "diagnostic-1" }
            : {
              kind: "full",
              resultId: "diagnostic-1",
              items: [{
                range: range(0, 2),
                severity: 2,
                code: "fake",
                source: "rust-analyzer",
                message: "warning",
              }],
            },
        }];
      }
      if (message.method === "textDocument/completion") {
        return [{
          jsonrpc: "2.0",
          id: message.id,
          result: {
            items: [
              {
                label: "println!",
                insertText: "println!(\"${1}\")",
                insertTextFormat: 2,
                detail: "macro",
              },
              {
                label: "mutating",
                additionalTextEdits: [{ range: range(0, 0), newText: "use x;" }],
              },
            ],
          },
        }];
      }
      if (message.method === "textDocument/hover") {
        return [{
          jsonrpc: "2.0",
          id: message.id,
          result: {
            contents: {
              kind: "markdown",
              value: "```rust\nfn main()\n```<script>bad()</script>",
            },
            range: range(0, 2),
          },
        }];
      }
      if (message.method === "textDocument/definition") {
        return [{
          jsonrpc: "2.0",
          id: message.id,
          result: [
            { uri: state.uri ?? this.uri, range: range(0, 2) },
            { uri: "file:///outside/lib.rs", range: range(0, 1) },
          ],
        }];
      }
      if (message.method === "textDocument/codeAction") {
        return [{
          jsonrpc: "2.0",
          id: message.id,
          result: [
            { title: "Apply edit", edit: { changes: {} } },
            { title: "Explain only" },
          ],
        }];
      }
      if (message.method === "textDocument/signatureHelp") {
        return [{
          jsonrpc: "2.0",
          id: message.id,
          result: {
            signatures: [{
              label: "drop(value: T)",
              parameters: [{ label: [5, 13], documentation: "value" }],
            }],
            activeSignature: 0,
            activeParameter: 0,
          },
        }];
      }
      if (message.method === "shutdown") {
        return [{ jsonrpc: "2.0", id: message.id, result: null }];
      }
      return null;
    },
  },

  typescript: {
    languageId: "typescript",
    relativePath: "src/main.ts",
    uri: "file:///workspace/src/main.ts",
    text: "const answer: number = 42;\n",
    capabilities: {
      textDocumentSync: { openClose: true, change: 2 },
      completionProvider: {
        triggerCharacters: [".", '"', "'", "/", "@", "<"],
      },
      hoverProvider: true,
      definitionProvider: true,
      codeActionProvider: true,
      signatureHelpProvider: { triggerCharacters: ["(", ",", "<"] },
      semanticTokensProvider: {
        full: true,
        legend: { tokenTypes: ["function"], tokenModifiers: ["declaration"] },
      },
    },
    respond(message, state) {
      if (message.method === "initialize") {
        return [baseInitialize(message.id, this.capabilities)];
      }
      if (message.method === "textDocument/semanticTokens/full") {
        state.uri = message.params?.textDocument?.uri ?? state.uri ?? this.uri;
        return [
          {
            jsonrpc: "2.0",
            method: "textDocument/publishDiagnostics",
            params: {
              uri: state.uri,
              version: 1,
              diagnostics: [{
                range: range(0, 5),
                severity: 1,
                code: "ts",
                source: "typescript",
                message: "fake error",
              }],
            },
          },
          {
            jsonrpc: "2.0",
            id: message.id,
            result: { data: [0, 0, 5, 0, 1] },
          },
        ];
      }
      if (message.method === "textDocument/completion") {
        return [{
          jsonrpc: "2.0",
          id: message.id,
          result: {
            items: [
              { label: "answer", insertText: "answer", detail: "const" },
              {
                label: "mutating",
                additionalTextEdits: [{ range: range(0, 0), newText: "import x;" }],
              },
            ],
          },
        }];
      }
      if (message.method === "textDocument/hover") {
        return [{
          jsonrpc: "2.0",
          id: message.id,
          result: {
            contents: {
              kind: "markdown",
              value: "```ts\nconst answer: number\n```<script>bad()</script>",
            },
            range: range(0, 5),
          },
        }];
      }
      if (message.method === "textDocument/definition") {
        return [{
          jsonrpc: "2.0",
          id: message.id,
          result: [
            { uri: state.uri ?? this.uri, range: range(0, 5) },
            { uri: "file:///outside/lib.ts", range: range(0, 1) },
          ],
        }];
      }
      if (message.method === "textDocument/codeAction") {
        return [{
          jsonrpc: "2.0",
          id: message.id,
          result: [
            { title: "Apply edit", edit: { changes: {} } },
            { title: "Explain only" },
          ],
        }];
      }
      if (message.method === "textDocument/signatureHelp") {
        return [{
          jsonrpc: "2.0",
          id: message.id,
          result: {
            signatures: [{
              label: "fn(value: number)",
              parameters: [{ label: [3, 15] }],
            }],
            activeSignature: 0,
            activeParameter: 0,
          },
        }];
      }
      if (message.method === "shutdown") {
        return [{ jsonrpc: "2.0", id: message.id, result: null }];
      }
      return null;
    },
  },

  javascript: {
    languageId: "javascript",
    relativePath: "src/main.js",
    uri: "file:///workspace/src/main.js",
    text: "const answer = 42;\n",
    capabilities: {
      textDocumentSync: { openClose: true, change: 2 },
      completionProvider: {
        triggerCharacters: [".", '"', "'", "/", "@", "<"],
      },
      hoverProvider: true,
      definitionProvider: true,
      codeActionProvider: true,
      signatureHelpProvider: { triggerCharacters: ["(", ",", "<"] },
      semanticTokensProvider: {
        full: true,
        legend: { tokenTypes: ["function"], tokenModifiers: ["declaration"] },
      },
    },
    respond(message, state) {
      // Reuse typescript responses with javascript identity defaults.
      return PROFILES.typescript.respond.call(this, message, state);
    },
  },

  markdown: {
    languageId: "markdown",
    relativePath: "README.md",
    uri: "file:///workspace/README.md",
    text: "# Title\n\nSee [[other]] and [[missing]].\n",
    capabilities: {
      textDocumentSync: { openClose: true, change: 1 },
      completionProvider: { triggerCharacters: ["[", "#", "("] },
      hoverProvider: true,
      definitionProvider: true,
      referencesProvider: true,
      renameProvider: true,
      codeActionProvider: { resolveProvider: false },
      semanticTokensProvider: {
        full: { delta: false },
        legend: {
          tokenTypes: ["class", "class", "enumMember"],
          tokenModifiers: [],
        },
      },
    },
    respond(message, state) {
      if (message.method === "initialize") {
        return [baseInitialize(message.id, this.capabilities)];
      }
      if (message.method === "textDocument/semanticTokens/full") {
        state.uri = message.params?.textDocument?.uri ?? state.uri ?? this.uri;
        return [
          {
            jsonrpc: "2.0",
            method: "textDocument/publishDiagnostics",
            params: {
              uri: state.uri,
              version: 1,
              diagnostics: [{
                range: { start: { line: 2, character: 6 }, end: { line: 2, character: 11 } },
                severity: 1,
                code: "2",
                source: "Marksman",
                message: "Link to non-existent document",
              }],
            },
          },
          {
            jsonrpc: "2.0",
            id: message.id,
            result: { data: [2, 6, 5, 0, 0] },
          },
        ];
      }
      if (message.method === "textDocument/completion") {
        return [{
          jsonrpc: "2.0",
          id: message.id,
          result: {
            isIncomplete: false,
            items: [{
              label: "other",
              textEdit: {
                range: range(6, 6, 2),
                newText: "[[other]]",
              },
            }],
          },
        }];
      }
      if (message.method === "textDocument/hover") {
        return [{
          jsonrpc: "2.0",
          id: message.id,
          result: {
            contents: {
              kind: "markdown",
              value: "# other<script>bad()</script>",
            },
            range: { start: { line: 2, character: 6 }, end: { line: 2, character: 11 } },
          },
        }];
      }
      if (message.method === "textDocument/definition") {
        return [{
          jsonrpc: "2.0",
          id: message.id,
          result: {
            uri: "file:///workspace/other.md",
            range: range(0, 7),
          },
        }];
      }
      if (message.method === "textDocument/codeAction") {
        return [{
          jsonrpc: "2.0",
          id: message.id,
          result: [{
            title: "Create a Table of Contents",
            edit: { changes: { [state.uri ?? this.uri]: [] } },
          }],
        }];
      }
      if (message.method === "shutdown") {
        return [{ jsonrpc: "2.0", id: message.id, result: null }];
      }
      return null;
    },
  },

  minimal: {
    languageId: "plaintext",
    relativePath: "note.txt",
    uri: "file:///workspace/note.txt",
    text: "hello\n",
    capabilities: {
      textDocumentSync: { openClose: true, change: 1 },
    },
    respond(message) {
      if (message.method === "initialize") {
        return [baseInitialize(message.id, this.capabilities)];
      }
      if (message.method === "shutdown") {
        return [{ jsonrpc: "2.0", id: message.id, result: null }];
      }
      return null;
    },
  },

  hung: {
    languageId: "plaintext",
    relativePath: "hang.txt",
    uri: "file:///workspace/hang.txt",
    text: "hang\n",
    capabilities: {
      textDocumentSync: { openClose: true, change: 1 },
      hoverProvider: true,
    },
    respond(message) {
      if (message.method === "initialize") {
        return [baseInitialize(message.id, this.capabilities)];
      }
      if (message.method === "shutdown") {
        return [{ jsonrpc: "2.0", id: message.id, result: null }];
      }
      // Never answer feature requests.
      return [];
    },
  },

  "exit-early": {
    languageId: "plaintext",
    relativePath: "exit.txt",
    uri: "file:///workspace/exit.txt",
    text: "exit\n",
    capabilities: {
      textDocumentSync: { openClose: true, change: 1 },
    },
    respond(message, state) {
      if (message.method === "initialize") {
        state.exitAfterWrite = true;
        return [baseInitialize(message.id, this.capabilities)];
      }
      return null;
    },
  },

  malformed: {
    languageId: "plaintext",
    relativePath: "bad.txt",
    uri: "file:///workspace/bad.txt",
    text: "bad\n",
    capabilities: {
      textDocumentSync: { openClose: true, change: 1 },
      hoverProvider: true,
    },
    respond(message, state) {
      if (message.method === "initialize") {
        return [baseInitialize(message.id, this.capabilities)];
      }
      if (message.method === "textDocument/hover") {
        state.emitRaw = "Content-Length: 3\r\n\r\n@@@";
        return [];
      }
      if (message.method === "shutdown") {
        return [{ jsonrpc: "2.0", id: message.id, result: null }];
      }
      return null;
    },
  },

  oversize: {
    languageId: "plaintext",
    relativePath: "big.txt",
    uri: "file:///workspace/big.txt",
    text: "big\n",
    capabilities: {
      textDocumentSync: { openClose: true, change: 1 },
      hoverProvider: true,
    },
    respond(message, state) {
      if (message.method === "initialize") {
        return [baseInitialize(message.id, this.capabilities)];
      }
      if (message.method === "textDocument/hover") {
        // 1 MiB + 1 body forces the shared frame ceiling.
        const body = JSON.stringify({
          jsonrpc: "2.0",
          id: message.id,
          result: { contents: "x".repeat(1024 * 1024) },
        });
        state.emitRaw = `Content-Length: ${Buffer.byteLength(body)}\r\n\r\n${body}`;
        return [];
      }
      if (message.method === "shutdown") {
        return [{ jsonrpc: "2.0", id: message.id, result: null }];
      }
      return null;
    },
  },
};

export function getProfile(name) {
  const profile = PROFILES[name];
  if (!profile) {
    throw new Error(`unknown fake LSP profile: ${name}`);
  }
  return profile;
}
