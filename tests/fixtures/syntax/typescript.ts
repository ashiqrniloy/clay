import { readFileSync } from "fs";
// syntax fixture

export interface Config {
  enabled: boolean;
  retries: number;
  onReady?: () => void;
}

const DEFAULT_CONFIG: Config = {
  enabled: true,
  retries: 3,
};

export class Loader {
  private cache = new Map<string, unknown>();

  public async load<T>(path: string): Promise<T | null> {
    if (this.cache.has(path)) {
      return this.cache.get(path) as T;
    }
    const raw = readFileSync(path, "utf8");
    const parsed = JSON.parse(raw) as T;
    this.cache.set(path, parsed);
    return parsed;
  }
}

const loader = new Loader();
const result = await loader.load<string>("./config.json");
console.log(result ?? "missing");
loader.cache.clear();
