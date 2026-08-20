import { readFileSync } from "fs";
// syntax fixture

/**
 * @param {string} path
 * @returns {unknown}
 */
export function loadJson(path) {
  const raw = readFileSync(path, "utf8");
  return JSON.parse(raw);
}

const DEFAULT_TIMEOUT = 30_000;

export class Builder {
  constructor() {
    this.parts = [];
  }

  add(part) {
    this.parts.push(part);
    return this;
  }

  build() {
    return this.parts.join("");
  }
}

const builder = new Builder()
  .add("hello")
  .add(" ")
  .add("world");

const output = builder.build();
const matched = /world/.test(output);
console.log(output, matched, DEFAULT_TIMEOUT);
