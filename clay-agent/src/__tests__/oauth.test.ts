import assert from "node:assert/strict";
import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import type { ExtensionKernel, OAuthLoginCallbacks } from "@arnilo/prism";
import { ClayAgentHost } from "../host.js";

async function tempDir(): Promise<string> {
  return mkdtemp(join(tmpdir(), "clay-agent-oauth-"));
}

function registerOauth(login: (callbacks?: OAuthLoginCallbacks) => Promise<{ access: string }>) {
  return (registries: ExtensionKernel["registries"]) => {
    registries.authMethods.register("xai\0oauth", {
      kind: "oauth",
      provider: "xai",
      oauth: { id: "xai", login },
    });
  };
}

test("oauthStart waits for a delayed device-code callback", async () => {
  const host = await ClayAgentHost.create({
    dataDir: await tempDir(),
    passphrase: "pass-phrase-ok",
    mock: true,
    registerModels: registerOauth(async (callbacks) => {
      await new Promise((resolve) => setTimeout(resolve, 75));
      await callbacks?.onDeviceCode?.({
        userCode: "ABCD-EFGH",
        verificationUri: "https://auth.x.ai/device",
      });
      return { access: "tok" };
    }),
  });
  const started = (await host.handle("credential.oauthStart", { provider: "xai" })) as {
    loginId: string;
    userCode?: string;
    verificationUri?: string;
  };
  assert.ok(started.loginId.length > 0);
  assert.equal(started.userCode, "ABCD-EFGH");
  assert.equal(started.verificationUri, "https://auth.x.ai/device");
  host.close();
});

test("oauthStart throws when login fails before a code", async () => {
  const host = await ClayAgentHost.create({
    dataDir: await tempDir(),
    passphrase: "pass-phrase-ok",
    mock: true,
    registerModels: registerOauth(async () => {
      throw new Error("device code failed: 401");
    }),
  });
  await assert.rejects(host.handle("credential.oauthStart", { provider: "xai" }), /device code failed: 401/);
  host.close();
});

test("oauthStart throws when login returns no device code or URL", async () => {
  const host = await ClayAgentHost.create({
    dataDir: await tempDir(),
    passphrase: "pass-phrase-ok",
    mock: true,
    registerModels: registerOauth(async () => ({ access: "tok" })),
  });
  await assert.rejects(
    host.handle("credential.oauthStart", { provider: "xai" }),
    /no device code or authorization URL/,
  );
  host.close();
});
