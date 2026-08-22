import type { CredentialValueSource, Extension, ExtensionKernel } from "@arnilo/prism";
import { createAlibabaProviderPackage } from "@arnilo/prism-provider-alibaba";
import { createAnthropicProviderPackage } from "@arnilo/prism-provider-anthropic";
import { createClinePassProviderPackage } from "@arnilo/prism-provider-clinepass";
import { createDeepSeekProviderPackage } from "@arnilo/prism-provider-deepseek";
import { createGoogleProviderPackage } from "@arnilo/prism-provider-google";
import { createKimiProviderPackage } from "@arnilo/prism-provider-kimi";
import { createNeuralWattProviderPackage } from "@arnilo/prism-provider-neuralwatt";
import { createOllamaProviderPackage } from "@arnilo/prism-provider-ollama";
import { createOpenAIProviderPackage } from "@arnilo/prism-provider-openai";
import { createOpenCodeGoProviderPackage } from "@arnilo/prism-provider-opencode-go";
import { createOpenRouterProviderPackage } from "@arnilo/prism-provider-openrouter";
import { createXaiProviderPackage } from "@arnilo/prism-provider-xai";
import { createZaiProviderPackage } from "@arnilo/prism-provider-zai";

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

/** Load first-party Prism 0.3.0 provider packages. Azure/Bedrock/Vertex need host
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
    hostConfigStub("@arnilo/prism-provider-azure", "azure", "credential"),
    hostConfigStub("@arnilo/prism-provider-bedrock", "bedrock", "credential"),
    hostConfigStub("@arnilo/prism-provider-vertex", "vertex", "credential"),
  ]);
}
