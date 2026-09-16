import type { CredentialValueSource, Extension, ExtensionKernel } from "@arnilo/prism";
import { createAlibabaProviderPackage } from "@arnilo/prism-providers/alibaba";
import { createAnthropicProviderPackage } from "@arnilo/prism-providers/anthropic";
import { createClinePassProviderPackage } from "@arnilo/prism-providers/clinepass";
import { createDeepSeekProviderPackage } from "@arnilo/prism-providers/deepseek";
import { createGoogleProviderPackage } from "@arnilo/prism-providers/google";
import { createKimiProviderPackage } from "@arnilo/prism-providers/kimi";
import { createNeuralWattProviderPackage } from "@arnilo/prism-providers/neuralwatt";
import { createOllamaProviderPackage } from "@arnilo/prism-providers/ollama";
import { createOpenAIProviderPackage } from "@arnilo/prism-providers/openai";
import { createOpenCodeGoProviderPackage } from "@arnilo/prism-providers/opencode-go";
import { createOpenRouterProviderPackage } from "@arnilo/prism-providers/openrouter";
import { createXaiProviderPackage } from "@arnilo/prism-providers/xai";
import { createZaiProviderPackage } from "@arnilo/prism-providers/zai";
import { createHyperProviderPackage } from "@arnilo/prism-providers/hyper";
import { createCommandCodeProviderPackage } from "@arnilo/prism-providers/commandcode";

function hostConfigStub(name: string, provider: string, credentialName: string): Extension {
  return {
    name,
    setup(api) {
      api.registerAuthMethod({
        kind: "api_key",
        provider,
        credentialName,
        metadata: { needsHostConfig: true },
      });
    },
  };
}

/** Load first-party Prism 0.7.0 provider packages. Azure/Bedrock/Vertex need host
 *  endpoint/region/project before their factories can run; stubs expose auth only. */
export async function loadProviderPackages(kernel: ExtensionKernel, apiKey: CredentialValueSource): Promise<void> {
  await kernel.load([
    createOpenAIProviderPackage({ apiKey, codexAccessToken: apiKey }),
    createAnthropicProviderPackage({ apiKey }),
    createGoogleProviderPackage({ apiKey }),
    createDeepSeekProviderPackage({ apiKey }),
    createXaiProviderPackage({ apiKey }),
    createClinePassProviderPackage({ apiKey }),
    createKimiProviderPackage({ kimiApiKey: apiKey }),
    createNeuralWattProviderPackage({ apiKey }),
    createOllamaProviderPackage({ apiKey }),
    createOpenCodeGoProviderPackage({ apiKey }),
    createZaiProviderPackage({ apiKey }),
    createAlibabaProviderPackage({ apiKey }),
    createOpenRouterProviderPackage({ apiKey }),
    createHyperProviderPackage({ apiKey }),
    createCommandCodeProviderPackage({ apiKey }),
    hostConfigStub("@arnilo/prism-providers/azure", "azure", "credential"),
    hostConfigStub("@arnilo/prism-providers/bedrock", "bedrock", "credential"),
    hostConfigStub("@arnilo/prism-providers/vertex", "vertex", "credential"),
  ]);
}