import { test } from "node:test";
import assert from "node:assert/strict";
import { CredentialStoreLockedError, CredentialStoreUnavailableError } from "@arnilo/prism-core/credentials/node";
import { resolveKeychain } from "../host.js";

type Probe = (req: { name: string; provider: string }) => Promise<unknown>;

/** Minimal KeychainCredentialStore-shaped fake; only `get` is exercised. */
function fakeKeychain(get: Probe) {
  return {
    service: "test",
    async get(req: { name: string; provider: string }) {
      return get(req);
    },
    async resolve() {
      return undefined;
    },
    async set() {},
    async delete() {
      return true;
    },
    async setOAuth() {},
    async getOAuth() {
      return undefined;
    },
    async deleteOAuth() {
      return true;
    },
    async list() {
      return [];
    },
    async listOAuth() {
      return [];
    },
  } as never;
}

test("resolveKeychain: healthy store (probe get resolves undefined) is kept", async () => {
  const store = fakeKeychain(async () => undefined);
  const result = await resolveKeychain(() => store);
  assert.equal(result, store);
});

test("resolveKeychain: locked/denied keychain fails closed (typed error surfaces)", async () => {
  const store = fakeKeychain(async () => {
    throw new CredentialStoreLockedError();
  });
  await assert.rejects(resolveKeychain(() => store), CredentialStoreLockedError);
});

test("resolveKeychain: unavailable backend degrades to vault-only (undefined, not an error)", async () => {
  const store = fakeKeychain(async () => {
    throw new CredentialStoreUnavailableError();
  });
  const result = await resolveKeychain(() => store);
  assert.equal(result, undefined);
});

test("resolveKeychain: missing credential stays undefined (probe is read-only, never writes)", async () => {
  const store = fakeKeychain(async () => undefined) as { get(req: { name: string; provider: string }): Promise<unknown> };
  assert.equal(await store.get({ name: "clay-probe", provider: "clay-probe" }), undefined);
});