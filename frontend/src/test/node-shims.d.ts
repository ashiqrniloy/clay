declare module "node:fs" {
  export interface Dirent {
    name: string;
    isDirectory(): boolean;
    isFile(): boolean;
  }
  export function readFileSync(path: string, encoding: string): string;
  export function readdirSync(
    path: string,
    options: { withFileTypes: true },
  ): Dirent[];
  export function existsSync(path: string): boolean;
}

declare module "node:path" {
  export function join(...paths: string[]): string;
  export function relative(from: string, to: string): string;
  export function resolve(...paths: string[]): string;
  export function dirname(path: string): string;
}

declare module "node:url" {
  export function fileURLToPath(url: string | URL): string;
}

declare namespace NodeJS {
  interface ProcessEnv {
    [key: string]: string | undefined;
  }
  interface Process {
    env: ProcessEnv;
  }
}

declare const process: NodeJS.Process;
