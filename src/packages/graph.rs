//! Declarative package graph relation helpers.
//!
//! Package graph evaluation is load/enable-time only. Manifests declare inert
//! relation data (`dependsOn`, `extends`, `disables`, `replaces`); the package
//! service applies this plan after capability grants are checked.

use crate::packages::manifest::PackageGraphRelations;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageGraphPlan {
    pub depends_on: Vec<String>,
    pub extends: Vec<String>,
    pub disables: Vec<String>,
    pub replaces: Vec<String>,
}

impl PackageGraphPlan {
    pub fn from_relations(relations: &PackageGraphRelations) -> Self {
        Self {
            depends_on: relations.depends_on.clone(),
            extends: relations.extends.clone(),
            disables: relations.disables.clone(),
            replaces: relations.replaces.clone(),
        }
    }

    pub fn requires_package_control(&self) -> bool {
        !self.disables.is_empty() || !self.replaces.is_empty()
    }

    pub fn activation_targets(&self) -> impl Iterator<Item = &String> {
        self.depends_on.iter().chain(self.extends.iter())
    }

    pub fn controlled_targets(&self) -> impl Iterator<Item = &String> {
        self.disables.iter().chain(self.replaces.iter())
    }

    pub fn all_targets(&self) -> impl Iterator<Item = &String> {
        self.activation_targets().chain(self.controlled_targets())
    }
}

pub fn cycle_from_stack(stack: &[String], package_name: &str) -> Vec<String> {
    let start = stack
        .iter()
        .position(|name| name == package_name)
        .unwrap_or(0);
    let mut cycle = stack[start..].to_vec();
    cycle.push(package_name.to_string());
    cycle
}
