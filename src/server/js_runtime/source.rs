// Auto-extracted from js_runtime.rs (Plan 090 task 3). Private submodule: source family.
use std::sync::Arc;

use deno_core::{
    ModuleLoadOptions, ModuleLoadReferrer, ModuleLoadResponse, ModuleLoader, ModuleSource,
    ModuleSourceCode, ModuleSpecifier, ModuleType, ResolutionKind, error::ModuleLoaderError,
};
use deno_error::JsErrorBox;

use crate::server::configuration::ConfigurationRuntime;
use crate::server::facades;
use crate::server::ops::PackageLoadEntryAllowlist;

pub(super) const CONTROLLED_MAIN_SPECIFIER: &str = "clay://runtime/main.js";
pub(super) const MARKDOWN_IT_MODULE_SPECIFIER: &str = "clay://vendor/markdown-it.js";

#[derive(Debug)]
pub(super) struct ClayModuleLoader {
    state: std::sync::Mutex<ClayModuleLoaderState>,
    // Shared validated package loadEntry gate. Populated by
    // `op_clay_packages_load_package_by_specifier`, checked in resolve/load.
    // Ceiling: one entry per loaded package module.
    package_load_entry_allowlist: Arc<PackageLoadEntryAllowlist>,
    // Trust domain this loader serves; gates facade and configuration-module
    // resolution.
    domain: crate::packages::bundled::RuntimeDomain,
}

#[derive(Debug)]
pub(super) struct ClayModuleLoaderState {
    main_specifier: ModuleSpecifier,
    main_source: Option<String>,
    configuration: Option<Arc<ConfigurationRuntime>>,
}

impl ClayModuleLoader {
    pub(super) fn new(
        main_specifier: ModuleSpecifier,
        main_source: Option<String>,
        configuration: Option<Arc<ConfigurationRuntime>>,
        package_load_entry_allowlist: Arc<PackageLoadEntryAllowlist>,
        domain: crate::packages::bundled::RuntimeDomain,
    ) -> Self {
        Self {
            state: std::sync::Mutex::new(ClayModuleLoaderState {
                main_specifier,
                main_source,
                configuration,
            }),
            package_load_entry_allowlist,
            domain,
        }
    }

    pub(super) fn set_entry(
        &self,
        main_specifier: ModuleSpecifier,
        main_source: Option<String>,
        configuration: Option<Arc<ConfigurationRuntime>>,
    ) {
        *self
            .state
            .lock()
            .expect("Clay module loader state mutex poisoned") = ClayModuleLoaderState {
            main_specifier,
            main_source,
            configuration,
        };
    }

    fn denied(specifier: &str) -> JsErrorBox {
        JsErrorBox::generic(format!(
            "runtime.invalid_import: module specifier `{specifier}` is not allowed in the server runtime boundary"
        ))
    }
}

impl ModuleLoader for ClayModuleLoader {
    fn resolve(
        &self,
        specifier: &str,
        referrer: &str,
        _kind: ResolutionKind,
    ) -> Result<ModuleSpecifier, ModuleLoaderError> {
        let state = self
            .state
            .lock()
            .expect("Clay module loader state mutex poisoned");
        if specifier == state.main_specifier.as_str() {
            return Ok(state.main_specifier.clone());
        }
        if facades::source(specifier).is_some() {
            if !facades::allowed(self.domain, specifier) {
                return Err(Self::denied(specifier));
            }
            return ModuleSpecifier::parse(specifier)
                .map_err(|error| Self::denied(&error.to_string()));
        }
        if specifier == "markdown-it" {
            return ModuleSpecifier::parse(MARKDOWN_IT_MODULE_SPECIFIER)
                .map_err(|error| Self::denied(&error.to_string()));
        }
        // Validated package `loadEntry`: opaque `clay://packages/...`
        // specifiers recorded by `op_clay_packages_load_package_by_specifier`.
        // This branch sits BEFORE the config-root branch because
        // `reject_non_local_specifier` would otherwise deny `clay://` URLs; the
        // shared allowlist is the single gate, so only a package module the
        // resolver op validated and recorded ever resolves here. Every other
        // `clay://packages/...` URL falls through to config-root confinement
        // (which rejects non-local specifiers) and the deny fallback.
        if self
            .package_load_entry_allowlist
            .absolute_path(specifier)
            .is_some()
        {
            return ModuleSpecifier::parse(specifier)
                .map_err(|error| Self::denied(&error.to_string()));
        }
        // Transitive relative imports from a validated package loadEntry are
        // confined to the validated package root by the allowlist and recorded
        // on first resolution. This lets a loadEntry import its own sibling
        // modules (e.g. `./index.js`) without weakening the config-root
        // boundary for any non-package specifier. ponytail: ceiling is the
        // validated package root; `resolve_relative` denies anything escaping it.
        if (specifier.starts_with("./") || specifier.starts_with("../"))
            && let Some(new_specifier) = self
                .package_load_entry_allowlist
                .resolve_relative(referrer, specifier)
        {
            return ModuleSpecifier::parse(&new_specifier)
                .map_err(|error| Self::denied(&error.to_string()));
        }
        // Trusted-only inventory helper exports (`lsp-shared/client.js`).
        // Exact specifier match; recorded under the helper root so relatives
        // stay confined. Third-party never sees this branch.
        if self.domain == crate::packages::bundled::RuntimeDomain::Trusted
            && let Some((export, absolute_path, helper_root)) =
                crate::packages::bundled::resolve_helper_export(specifier)
        {
            let opaque = format!(
                "clay://packages/{}/{}",
                export.root,
                export.file.replace('\\', "/")
            );
            self.package_load_entry_allowlist.record_for_package(
                &opaque,
                absolute_path,
                helper_root,
                Some(export.root),
            );
            return ModuleSpecifier::parse(&opaque)
                .map_err(|error| Self::denied(&error.to_string()));
        }
        if self.domain == crate::packages::bundled::RuntimeDomain::Trusted
            && let Some(configuration) = &state.configuration
        {
            return configuration
                .resolve_module(specifier, referrer)
                .map_err(|error| error.to_js_error());
        }

        Err(Self::denied(&format!("{specifier} from {referrer}")))
    }

    fn load(
        &self,
        module_specifier: &ModuleSpecifier,
        _maybe_referrer: Option<&ModuleLoadReferrer>,
        _options: ModuleLoadOptions,
    ) -> ModuleLoadResponse {
        let state = self
            .state
            .lock()
            .expect("Clay module loader state mutex poisoned");
        if module_specifier == &state.main_specifier
            && let Some(source) = &state.main_source
        {
            return ModuleLoadResponse::Sync(Ok(ModuleSource::new(
                ModuleType::JavaScript,
                ModuleSourceCode::String(source.clone().into()),
                module_specifier,
                None,
            )));
        }

        if let Some(source) = facades::source(module_specifier.as_str()) {
            if !facades::allowed(self.domain, module_specifier.as_str()) {
                return ModuleLoadResponse::Sync(Err(Self::denied(module_specifier.as_str())));
            }
            return ModuleLoadResponse::Sync(Ok(ModuleSource::new(
                ModuleType::JavaScript,
                ModuleSourceCode::String(source.to_string().into()),
                module_specifier,
                None,
            )));
        }

        if module_specifier.as_str() == MARKDOWN_IT_MODULE_SPECIFIER {
            return ModuleLoadResponse::Sync(markdown_it_module_source().map(|source| {
                ModuleSource::new(
                    ModuleType::JavaScript,
                    ModuleSourceCode::String(source.into()),
                    module_specifier,
                    None,
                )
            }));
        }
        // Validated package `loadEntry`: read the on-disk source the resolver op
        // recorded for this exact opaque specifier. Single gate, same allowlist
        // as `resolve`; no path outside the validated package root is ever read.
        if let Some(absolute_path) = self
            .package_load_entry_allowlist
            .absolute_path(module_specifier.as_str())
        {
            return ModuleLoadResponse::Sync(
                std::fs::read_to_string(&absolute_path)
                    .map_err(|error| {
                        Self::denied(&format!(
                            "package loadEntry {module_specifier} could not be loaded ({error})"
                        ))
                    })
                    .map(|source| {
                        ModuleSource::new(
                            ModuleType::JavaScript,
                            ModuleSourceCode::String(source.into()),
                            module_specifier,
                            None,
                        )
                    }),
            );
        }
        if self.domain == crate::packages::bundled::RuntimeDomain::Trusted
            && let Some(configuration) = &state.configuration
        {
            return ModuleLoadResponse::Sync(
                configuration
                    .load_module_source(module_specifier)
                    .map(|source| {
                        ModuleSource::new(
                            ModuleType::JavaScript,
                            ModuleSourceCode::String(source.into()),
                            module_specifier,
                            None,
                        )
                    })
                    .map_err(|error| error.to_js_error()),
            );
        }

        ModuleLoadResponse::Sync(Err(Self::denied(module_specifier.as_str())))
    }
}

pub(super) fn markdown_it_module_source() -> Result<String, ModuleLoaderError> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("packages")
        .join("markdown")
        .join("node_modules")
        .join("markdown-it")
        .join("dist")
        .join("markdown-it.js");
    let bundled = std::fs::read_to_string(&path).map_err(|error| {
        ClayModuleLoader::denied(&format!(
            "markdown-it bundle could not be loaded from {} ({error})",
            path.display()
        ))
    })?;
    Ok(format!(
        "{bundled}\nconst MarkdownIt = globalThis.markdownit;\nexport default MarkdownIt;\nexport {{ MarkdownIt }};\n"
    ))
}
