# @arnilo/clay

npm wrapper for the [Clay coding assistant](https://clay.dev). The `clay`
command execs a native binary shipped in an optional platform package
(`@arnilo/clay-linux-x64`, …). This package ships no native code, runs no
lifecycle scripts, and makes no network requests at install time.

Prefer the curl installer for self-updating installs:

```sh
curl -fsSL https://clay.dev/install.sh | sh
```

## Warning

Clay packages run with full system access — review before installing.
`clay install` never executes package code; `clay package adopt` is the
reviewable gate.
