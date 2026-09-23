// Plan 136 task 13 fixture config: the capability grant is written by
// configuration instead of the CLI. `packages.authorize` is reachable only from
// the trusted domain, so this file (and every other init.js) is the only
// JavaScript that can call it — package code cannot import `clay:packages`.
//
// The grant is durable: it lands in the store's approval record, so a separate
// `clay` process sees it and a later launch can load the package. The package
// also has to be enabled, which is a host verb (`clay package enable`), so the
// load below fails closed until that has happened once.
import { authorize, loadPackage } from "clay:packages";

await authorize({
    package: "@fixture/lane",
    capabilities: ["completion-provider", "mode-registration", "parse-document"],
    runtimeProfile: "native-trust",
    approvedBy: "config",
});

await loadPackage("@fixture/lane");
