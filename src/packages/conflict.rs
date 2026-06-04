//! Deterministic cross-package conflict detection for enabled packages.
//!
//! Conflict checks run at package enable/reload time only.  The pass builds
//! sorted contribution indices from the enabled package set and returns the
//! first deterministic, provenance-preserving diagnostic; it never silently
//! overrides package behavior.
use std::collections::BTreeMap;

use crate::packages::record::PackageRecord;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageConflictDiagnostic {
    pub kind: PackageConflictKind,
    pub contribution_id: String,
    pub first: PackageConflictProvenance,
    pub second: PackageConflictProvenance,
    pub message: String,
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
    BehaviorManifestEntryCollision,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageConflictProvenance {
    pub package_name: String,
    pub package_version: String,
    pub api_prefix: String,
}

impl PackageConflictProvenance {
    fn from_record(record: &PackageRecord) -> Self {
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
                    key_id,
                    prov.clone(),
                    PackageConflictKind::AmbiguousKeyBinding,
                    "ambiguous key binding across packages without a distinct priority/routing policy",
                )?;
                insert_unique(
                    &mut behavior_entries,
                    format!("key:{}", binding),
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
            contribution_id: id.clone(),
            first: first.clone(),
            second: provenance.clone(),
            message: format!(
                "{label} `{id}` between `{}` ({}) and `{}` ({})",
                first.package_name,
                first.api_prefix,
                provenance.package_name,
                provenance.api_prefix
            ),
        });
    }
    index.insert(id, provenance);
    Ok(())
}
