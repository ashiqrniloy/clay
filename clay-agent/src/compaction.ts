import {
  type AIProvider,
  type CompactionStrategy,
  type ExtensionKernel,
  type ModelConfig,
  createDefaultCompactionStrategy,
} from "@arnilo/prism";
import { createLlmCompactionStrategy } from "@arnilo/prism-memory/compaction/llm";
import { createObservationalMemoryCompactionStrategy } from "@arnilo/prism-memory/compaction/observational-memory";

export const COMPACTION_STRATEGIES = ["default", "llm", "om"] as const;
export type CompactionStrategyName = (typeof COMPACTION_STRATEGIES)[number];

/** Clay default (2158). Prism's package default is 81000. */
export const DEFAULT_COMPACT_AFTER_TOKENS = 80_000;

export function isCompactionStrategyName(value: string): value is CompactionStrategyName {
  return (COMPACTION_STRATEGIES as readonly string[]).includes(value);
}

export interface CompactionFactoryOptions {
  readonly secrets: readonly string[];
  readonly llm?: { readonly provider: AIProvider; readonly model: ModelConfig };
}

export function createNamedCompactionStrategy(
  name: CompactionStrategyName,
  options: CompactionFactoryOptions,
): CompactionStrategy {
  switch (name) {
    case "default":
      return createDefaultCompactionStrategy({ name: "default", secrets: options.secrets });
    case "llm":
      if (!options.llm) {
        return {
          name: "llm",
          compact() {
            throw new Error("llm compaction requires a summary provider");
          },
        };
      }
      return createLlmCompactionStrategy({
        name: "llm",
        provider: options.llm.provider,
        model: options.llm.model,
        secrets: options.secrets,
      });
    case "om":
      return createObservationalMemoryCompactionStrategy({ name: "om", secrets: options.secrets });
  }
}

export function registerCompactionStrategies(kernel: ExtensionKernel, options: CompactionFactoryOptions): void {
  for (const name of COMPACTION_STRATEGIES) {
    kernel.registries.compactionStrategies.register(name, createNamedCompactionStrategy(name, options));
  }
}
