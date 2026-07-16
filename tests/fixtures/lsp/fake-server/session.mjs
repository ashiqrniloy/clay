import { pathToFileURL } from "node:url";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { getProfile, PROFILES } from "./profiles.mjs";

const sharedRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "../../../../packages/lsp-shared",
);
const { encodeFrame, FrameDecoder } = await import(
  pathToFileURL(path.join(sharedRoot, "framing.js")).href
);

/**
 * In-process standards-shaped LSP fake session.
 * Configure with a named profile from `./profiles.mjs`.
 */
export class FakeLspSession {
  constructor(profileName = "rust", options = {}) {
    this.profileName = profileName;
    this.profile = getProfile(profileName);
    this.decoder = new FrameDecoder();
    this.reads = [];
    this.messages = [];
    this.stopped = false;
    this.state = {
      uri: options.uri ?? this.profile.uri,
      semanticRequests: 0,
    };
    this.fragmentReads = options.fragmentReads !== false;
    this.onExit = options.onExit;
  }

  get semanticRequests() {
    return this.state.semanticRequests ?? 0;
  }

  get uri() {
    return this.state.uri;
  }

  set uri(value) {
    this.state.uri = value;
  }

  queue(messageOrBytes) {
    if (typeof messageOrBytes === "string" || messageOrBytes instanceof Uint8Array) {
      const bytes = typeof messageOrBytes === "string"
        ? Buffer.from(messageOrBytes)
        : messageOrBytes;
      if (this.fragmentReads) {
        for (let index = 0; index < bytes.length; index += 3) {
          this.reads.push(bytes.slice(index, index + 3));
        }
      } else {
        this.reads.push(bytes instanceof Uint8Array ? bytes : new Uint8Array(bytes));
      }
      return;
    }
    this.queue(encodeFrame(messageOrBytes));
  }

  async sendBytes(bytes) {
    for (const message of this.decoder.push(bytes)) {
      this.messages.push(message);
      if (!("method" in message)) continue;
      if (!("id" in message) && message.method === "exit") {
        this.stopped = true;
        this.onExit?.();
        continue;
      }
      if (!("id" in message)) continue;
      const responses = this.profile.respond(message, this.state);
      if (this.state.emitRaw) {
        this.queue(this.state.emitRaw);
        this.state.emitRaw = undefined;
      }
      if (!responses) {
        throw new Error(`unexpected request ${message.method} for profile ${this.profileName}`);
      }
      for (const response of responses) this.queue(response);
      if (this.state.exitAfterWrite) {
        this.stopped = true;
        this.onExit?.();
      }
    }
  }

  async readBytes() {
    if (this.stopped && this.reads.length === 0) return new Uint8Array();
    return this.reads.shift() ?? new Uint8Array();
  }

  async stop() {
    this.stopped = true;
  }
}

export function profileNames() {
  return Object.keys(PROFILES);
}

export { getProfile, encodeFrame, FrameDecoder, PROFILES };
