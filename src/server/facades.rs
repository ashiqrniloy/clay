use crate::packages::bundled::RuntimeDomain;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FacadeAccess {
    TrustedOnly,
    Public,
}

#[derive(Clone, Copy, Debug)]
struct Facade {
    specifier: &'static str,
    source: &'static str,
    access: FacadeAccess,
}

impl Facade {
    const fn trusted(specifier: &'static str, source: &'static str) -> Self {
        Self {
            specifier,
            source,
            access: FacadeAccess::TrustedOnly,
        }
    }

    const fn public(specifier: &'static str, source: &'static str) -> Self {
        Self {
            specifier,
            source,
            access: FacadeAccess::Public,
        }
    }
}

const FACADES: &[Facade] = &[
    Facade::trusted(
        "clay:configuration",
        include_str!("../../runtime/js/configuration.js"),
    ),
    Facade::public("clay:sdui", include_str!("../../runtime/js/sdui.js")),
    Facade::public("clay:ui", include_str!("../../runtime/js/ui.js")),
    Facade::trusted(
        "clay:documents",
        include_str!("../../runtime/js/documents.js"),
    ),
    Facade::trusted(
        "clay:workspace",
        include_str!("../../runtime/js/workspace.js"),
    ),
    Facade::public("clay:git", include_str!("../../runtime/js/git.js")),
    Facade::trusted(
        "clay:keybindings",
        include_str!("../../runtime/js/keybindings.js"),
    ),
    Facade::public(
        "clay:behavior",
        include_str!("../../runtime/js/behavior.js"),
    ),
    Facade::trusted(
        "clay:packages",
        include_str!("../../runtime/js/packages.js"),
    ),
    Facade::public(
        "clay:language-server",
        include_str!("../../runtime/js/language-server.js"),
    ),
    Facade::public("clay:modes", include_str!("../../runtime/js/modes.js")),
    Facade::public(
        "clay:commands",
        include_str!("../../runtime/js/commands.js"),
    ),
    Facade::public(
        "clay:decorations",
        include_str!("../../runtime/js/decorations.js"),
    ),
    Facade::public(
        "clay:diagnostics",
        include_str!("../../runtime/js/diagnostics.js"),
    ),
    Facade::public("clay:parse", include_str!("../../runtime/js/parse.js")),
    Facade::public("clay:syntax", include_str!("../../runtime/js/syntax.js")),
    Facade::public(
        "clay:completion",
        include_str!("../../runtime/js/completion.js"),
    ),
    Facade::public(
        "clay:language",
        include_str!("../../runtime/js/language.js"),
    ),
    Facade::trusted(
        "clay:application",
        include_str!("../../runtime/js/application.js"),
    ),
    Facade::trusted("clay:editor", include_str!("../../runtime/js/editor.js")),
    Facade::trusted("clay:shell", include_str!("../../runtime/js/shell.js")),
    Facade::trusted("clay:theme", include_str!("../../runtime/js/theme.js")),
];

pub(super) fn source(specifier: &str) -> Option<&'static str> {
    FACADES
        .iter()
        .find(|facade| facade.specifier == specifier)
        .map(|facade| facade.source)
}

pub(super) fn allowed(domain: RuntimeDomain, specifier: &str) -> bool {
    FACADES.iter().any(|facade| {
        facade.specifier == specifier
            && (domain == RuntimeDomain::Trusted || facade.access == FacadeAccess::Public)
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn facade_inventory_is_unique_and_domain_partitioned() {
        let specifiers: HashSet<_> = FACADES.iter().map(|facade| facade.specifier).collect();
        assert_eq!(specifiers.len(), 22);
        assert_eq!(
            FACADES
                .iter()
                .filter(|facade| facade.access == FacadeAccess::Public)
                .count(),
            13
        );
    }
}
