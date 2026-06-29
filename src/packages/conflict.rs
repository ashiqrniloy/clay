//! Deterministic cross-package conflict detection for enabled packages.
//!
//! Conflict checks run at package enable/reload time only.  The pass builds
//! sorted contribution indices from the enabled package set and returns the
//! first deterministic, provenance-preserving diagnostic; it never silently
//! overrides package behavior.
use std::collections::{BTreeMap, BTreeSet};

use crate::packages::record::PackageRecord;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageConflictDiagnostic {
    pub kind: PackageConflictKind,
    pub contribution_id: Box<str>,
    pub first: Box<PackageConflictProvenance>,
    pub second: Box<PackageConflictProvenance>,
    pub message: Box<str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageConflictKind {
    DuplicatePrefix,
    DuplicateMode,
    DuplicateCommand,
    AmbiguousKeyBinding,
    ConfigurationKeyCollision,
    SduiRegionCollision,
    DecorationPrimitiveCollision,
    UiPanelCollision,
    UiFixedSlotCollision,
    UiComponentCollision,
    UiOverlayCollision,
    ThemeTokenCollision,
    InputContributionCollision,
    UiStateScopeCollision,
    LayoutOverrideCollision,
    PackageOptionCollision,
    BehaviorManifestEntryCollision,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageConflictProvenance {
    pub package_name: String,
    pub package_version: String,
    pub api_prefix: String,
}

impl PackageConflictProvenance {
    pub(crate) fn from_record(record: &PackageRecord) -> Self {
        Self {
            package_name: record.manifest.name.clone(),
            package_version: record.manifest.version.clone(),
            api_prefix: record.manifest.clay.api_prefix.clone(),
        }
    }
}

impl std::fmt::Display for PackageConflictDiagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for PackageConflictDiagnostic {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageConflictResolutionDiagnostic {
    pub contribution_id: Box<str>,
    pub winner: PackageConflictProvenance,
    pub loser: PackageConflictProvenance,
    pub reason: PackageConflictResolutionReason,
    pub message: Box<str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageConflictResolutionReason {
    UserOverride,
    PackageReplaces,
    PackageDisables,
}

impl PackageConflictResolutionReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::UserOverride => "user-override",
            Self::PackageReplaces => "package-replaces",
            Self::PackageDisables => "package-disables",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PackageConflictResolutionPolicy {
    user_overrides: BTreeMap<String, String>,
}

impl PackageConflictResolutionPolicy {
    pub fn set_user_override(
        &mut self,
        contribution_id: impl Into<String>,
        winner: impl Into<String>,
    ) {
        self.user_overrides
            .insert(contribution_id.into(), winner.into());
    }

    pub fn user_override_winner(&self, contribution_id: &str) -> Option<&str> {
        self.user_overrides.get(contribution_id).map(String::as_str)
    }
}

pub fn check_enabled_packages_with_policy<'a>(
    enabled: impl IntoIterator<Item = &'a PackageRecord>,
    policy: &PackageConflictResolutionPolicy,
) -> Result<Vec<PackageConflictResolutionDiagnostic>, PackageConflictDiagnostic> {
    let mut records: Vec<&PackageRecord> = enabled.into_iter().collect();
    let mut resolutions = Vec::new();

    loop {
        match check_enabled_packages(records.iter().copied()) {
            Ok(()) => return Ok(resolutions),
            Err(conflict) => {
                let Some(winner) = policy.user_override_winner(&conflict.contribution_id) else {
                    return Err(conflict);
                };
                let loser = if conflict.first.package_name == winner {
                    conflict.second.package_name.clone()
                } else if conflict.second.package_name == winner {
                    conflict.first.package_name.clone()
                } else {
                    return Err(conflict);
                };
                records.retain(|record| record.manifest.name != loser);
                resolutions.push(PackageConflictResolutionDiagnostic {
                    contribution_id: conflict.contribution_id.clone(),
                    winner: if conflict.first.package_name == winner {
                        (*conflict.first).clone()
                    } else {
                        (*conflict.second).clone()
                    },
                    loser: if conflict.first.package_name == loser {
                        (*conflict.first).clone()
                    } else {
                        (*conflict.second).clone()
                    },
                    reason: PackageConflictResolutionReason::UserOverride,
                    message: format!(
                        "user conflict override selected `{winner}` for `{}` and disabled `{loser}`",
                        conflict.contribution_id
                    )
                    .into_boxed_str(),
                });
            }
        }
    }
}

pub fn unresolved_conflict_after_removing_records<'a>(
    enabled: impl IntoIterator<Item = &'a PackageRecord>,
    removed: &BTreeSet<String>,
) -> Result<(), PackageConflictDiagnostic> {
    check_enabled_packages(
        enabled
            .into_iter()
            .filter(|record| !removed.contains(&record.manifest.name)),
    )
}

/// Check all enabled package records for deterministic conflicts.
pub fn check_enabled_packages<'a>(
    enabled: impl IntoIterator<Item = &'a PackageRecord>,
) -> Result<(), PackageConflictDiagnostic> {
    let mut records: Vec<&PackageRecord> = enabled.into_iter().collect();
    records.sort_by(|a, b| {
        a.manifest
            .clay
            .api_prefix
            .cmp(&b.manifest.clay.api_prefix)
            .then_with(|| a.manifest.name.cmp(&b.manifest.name))
            .then_with(|| a.manifest.version.cmp(&b.manifest.version))
    });

    let mut prefixes = BTreeMap::new();
    let mut modes = BTreeMap::new();
    let mut commands = BTreeMap::new();
    let mut key_bindings = BTreeMap::new();
    let mut config_keys = BTreeMap::new();
    let mut sdui_regions = BTreeMap::new();
    let mut decorations = BTreeMap::new();
    let mut ui_panels = BTreeMap::new();
    let mut ui_fixed_slots = BTreeMap::new();
    let mut ui_components = BTreeMap::new();
    let mut ui_overlays = BTreeMap::new();
    let mut theme_tokens = BTreeMap::new();
    let mut input_contributions = BTreeMap::new();
    let mut ui_state_scopes = BTreeMap::new();
    let mut layout_overrides = BTreeMap::new();
    let mut package_options = BTreeMap::new();
    let mut behavior_entries = BTreeMap::new();

    for record in records {
        let prov = PackageConflictProvenance::from_record(record);
        for mode in &record.manifest.clay.modes {
            insert_unique(
                &mut modes,
                mode.clone(),
                prov.clone(),
                PackageConflictKind::DuplicateMode,
                "duplicate mode name",
            )?;
        }
        for command in &record.contributions.commands {
            insert_unique(
                &mut commands,
                command.id.clone(),
                prov.clone(),
                PackageConflictKind::DuplicateCommand,
                "duplicate command ID",
            )?;
            insert_unique(
                &mut behavior_entries,
                format!("command:{}", command.id),
                prov.clone(),
                PackageConflictKind::BehaviorManifestEntryCollision,
                "duplicate behavior manifest command entry",
            )?;
        }
        for key in &record.contributions.key_routing {
            if let Some(binding) = &key.key_binding {
                let priority = key.priority.unwrap_or(0);
                let key_id = format!(
                    "{}:{}:{}",
                    binding,
                    key.routing_policy.as_deref().unwrap_or(""),
                    priority
                );
                insert_unique(
                    &mut key_bindings,
                    key_id.clone(),
                    prov.clone(),
                    PackageConflictKind::AmbiguousKeyBinding,
                    "ambiguous key binding across packages without a distinct priority/routing policy",
                )?;
                insert_unique(
                    &mut behavior_entries,
                    format!("key:{key_id}"),
                    prov.clone(),
                    PackageConflictKind::BehaviorManifestEntryCollision,
                    "duplicate behavior manifest key binding entry",
                )?;
            }
        }
        for config in &record.contributions.configuration {
            insert_unique(
                &mut config_keys,
                config.key.clone(),
                prov.clone(),
                PackageConflictKind::ConfigurationKeyCollision,
                "configuration key collision",
            )?;
        }
        for sdui in &record.contributions.sdui {
            insert_unique(
                &mut sdui_regions,
                sdui.region_id.clone(),
                prov.clone(),
                PackageConflictKind::SduiRegionCollision,
                "SDUI region/slot collision",
            )?;
        }
        for decoration in &record.contributions.decorations {
            insert_unique(
                &mut decorations,
                decoration.primitive_id.clone(),
                prov.clone(),
                PackageConflictKind::DecorationPrimitiveCollision,
                "decoration/render primitive collision",
            )?;
        }
        for panel in &record.contributions.ui_panels {
            insert_unique(
                &mut ui_panels,
                panel.id.clone(),
                prov.clone(),
                PackageConflictKind::UiPanelCollision,
                "package UI panel ID collision",
            )?;
            insert_unique(
                &mut ui_fixed_slots,
                panel.slot.clone(),
                prov.clone(),
                PackageConflictKind::UiFixedSlotCollision,
                "package UI fixed slot collision",
            )?;
        }
        for component in &record.contributions.ui_components {
            insert_unique(
                &mut ui_components,
                component.id.clone(),
                prov.clone(),
                PackageConflictKind::UiComponentCollision,
                "package UI component ID collision",
            )?;
        }
        for overlay in &record.contributions.ui_overlays {
            insert_unique(
                &mut ui_overlays,
                overlay.id.clone(),
                prov.clone(),
                PackageConflictKind::UiOverlayCollision,
                "package UI overlay ID collision",
            )?;
        }
        for token in &record.contributions.theme_tokens {
            insert_unique(
                &mut theme_tokens,
                token.token.clone(),
                prov.clone(),
                PackageConflictKind::ThemeTokenCollision,
                "package theme token collision",
            )?;
        }
        for input in &record.contributions.input_contributions {
            insert_unique(
                &mut input_contributions,
                input.id.clone(),
                prov.clone(),
                PackageConflictKind::InputContributionCollision,
                "package input contribution collision",
            )?;
        }
        for state in &record.contributions.ui_state_scopes {
            insert_unique(
                &mut ui_state_scopes,
                state.id.clone(),
                prov.clone(),
                PackageConflictKind::UiStateScopeCollision,
                "package UI state scope collision",
            )?;
        }
        for layout in &record.contributions.layout_overrides {
            insert_unique(
                &mut layout_overrides,
                format!("{}:{}", layout.target_id, layout.property),
                prov.clone(),
                PackageConflictKind::LayoutOverrideCollision,
                "package layout override collision",
            )?;
        }
        for option in &record.contributions.package_options {
            insert_unique(
                &mut package_options,
                option.option.clone(),
                prov.clone(),
                PackageConflictKind::PackageOptionCollision,
                "package option schema collision",
            )?;
        }
        for transform in &record.contributions.text_transforms {
            insert_unique(
                &mut behavior_entries,
                format!("transform:{}", transform.transform_id),
                prov.clone(),
                PackageConflictKind::BehaviorManifestEntryCollision,
                "duplicate behavior manifest text transform entry",
            )?;
        }
        insert_unique(
            &mut prefixes,
            record.manifest.clay.api_prefix.clone(),
            prov,
            PackageConflictKind::DuplicatePrefix,
            "duplicate package prefix",
        )?;
    }

    Ok(())
}

fn insert_unique(
    index: &mut BTreeMap<String, PackageConflictProvenance>,
    id: String,
    provenance: PackageConflictProvenance,
    kind: PackageConflictKind,
    label: &'static str,
) -> Result<(), PackageConflictDiagnostic> {
    if let Some(first) = index.get(&id) {
        return Err(PackageConflictDiagnostic {
            kind,
            contribution_id: id.clone().into_boxed_str(),
            first: Box::new(first.clone()),
            second: Box::new(provenance.clone()),
            message: format!(
                "{label} `{id}` between `{}` ({}) and `{}` ({})",
                first.package_name,
                first.api_prefix,
                provenance.package_name,
                provenance.api_prefix
            )
            .into_boxed_str(),
        });
    }
    index.insert(id, provenance);
    Ok(())
}
