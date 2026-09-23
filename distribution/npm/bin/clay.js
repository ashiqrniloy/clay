#!/usr/bin/env node
// @arnilo/clay bin shim — execs the native clay binary shipped by an
// optional platform package (@arnilo/clay-<platform>-<arch>). Never
// downloads anything at run time; no lifecycle scripts run at install
// time either. Native binaries arrive only via optionalDependencies.
"use strict";

const { spawn } = require("node:child_process");
const path = require("node:path");
const fs = require("node:fs");

const PLATFORM_PACKAGES = {
  "linux-x64": "@arnilo/clay-linux-x64",
  "linux-arm64": "@arnilo/clay-linux-arm64",
};

function nativePackage() {
  const suffix = `${process.platform}-${process.arch}`;
  const name = PLATFORM_PACKAGES[suffix];
  if (!name) return null;
  try {
    return path.join(
      path.dirname(require.resolve(`${name}/package.json`)),
      "bin",
      "clay",
    );
  } catch {
    return null;
  }
}

const native = nativePackage();
if (!native || !fs.existsSync(native)) {
  console.error(
    "clay: no native binary for this platform. Install via the curl installer instead: https://clay.dev/install (see docs/development/distribution.md).",
  );
  process.exit(1);
}

const child = spawn(native, process.argv.slice(2), { stdio: "inherit" });
child.on("error", (error) => {
  console.error(`clay: failed to exec native binary: ${error.message}`);
  process.exit(1);
});
child.on("exit", (code) => process.exit(code ?? 1));
