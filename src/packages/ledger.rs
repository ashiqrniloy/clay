//! Durable Clay-owned install ledger (plan 115 task 3).
//!
//! One record per Clay-initiated install, distinct from the approval store.
//! Authority for pinned-vs-floating update skips and list provenance.
//! Manager-discovered packages with no entry are unmanaged and never
//! auto-updated.
//!
//! File: `<store-root>/installs.json`, Unix `0o600`, atomic write
//! (temp + fsync + rename). Load fails closed on corruption, unknown
//! version, oversize, or unsafe permissions.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::packages::approvals::atomic_write_owner_only;

pub const INSTALL_LEDGER_FILE_NAME: &str = "installs.json";
const LEDGER_VERSION: u64 = 1;
const MAX_LEDGER_BYTES: usize = 256 * 1024;
const MAX_LEDGER_RECORDS: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallRecord {
    pub name: String,
    pub spec: String,
    pub pinned: bool,
    pub version: String,
    pub source: String,
    pub installed_at: String,
}

#[derive(Debug)]
pub enum InstallLedgerError {
    Corrupt { reason: String },
    UnsafePermissions,
    Io(std::io::Error),
}

impl std::fmt::Display for InstallLedgerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Corrupt { reason } => {
                write!(f, "packages.install_ledger.corrupt: {reason}")
            }
            Self::UnsafePermissions => write!(
                f,
                "packages.install_ledger.unsafe_permissions: install ledger must be owner-only"
            ),
            Self::Io(error) => write!(f, "packages.install_ledger.io: {error}"),
        }
    }
}

impl std::error::Error for InstallLedgerError {}

#[derive(Debug)]
pub struct InstallLedger {
    path: Option<PathBuf>,
    records: HashMap<String, InstallRecord>,
}

impl InstallLedger {
    pub fn in_memory() -> Self {
        Self {
            path: None,
            records: HashMap::new(),
        }
    }

    pub fn open(store_root: &Path) -> Result<Self, InstallLedgerError> {
        let path = store_root.join(INSTALL_LEDGER_FILE_NAME);
        let metadata = match fs::metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self {
                    path: Some(path),
                    records: HashMap::new(),
                });
            }
            Err(error) => return Err(InstallLedgerError::Io(error)),
        };
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if metadata.permissions().mode() & 0o077 != 0 {
                return Err(InstallLedgerError::UnsafePermissions);
            }
        }
        if metadata.len() as usize > MAX_LEDGER_BYTES {
            return Err(InstallLedgerError::Corrupt {
                reason: format!("ledger exceeds {MAX_LEDGER_BYTES} bytes"),
            });
        }
        let bytes = fs::read(&path).map_err(InstallLedgerError::Io)?;
        let document: Value =
            serde_json::from_slice(&bytes).map_err(|error| InstallLedgerError::Corrupt {
                reason: format!("invalid ledger JSON: {error}"),
            })?;
        let version = document.get("version").and_then(Value::as_u64);
        if version != Some(LEDGER_VERSION) {
            return Err(InstallLedgerError::Corrupt {
                reason: format!("unknown ledger version {version:?} (expected {LEDGER_VERSION})"),
            });
        }
        let packages = document
            .get("packages")
            .and_then(Value::as_array)
            .ok_or_else(|| InstallLedgerError::Corrupt {
                reason: "ledger `packages` must be an array".to_string(),
            })?;
        if packages.len() > MAX_LEDGER_RECORDS {
            return Err(InstallLedgerError::Corrupt {
                reason: format!("ledger exceeds {MAX_LEDGER_RECORDS} records"),
            });
        }
        let mut records = HashMap::with_capacity(packages.len());
        for value in packages {
            let record = InstallRecord::from_json(value)?;
            if records.insert(record.name.clone(), record).is_some() {
                return Err(InstallLedgerError::Corrupt {
                    reason: "duplicate install record".to_string(),
                });
            }
        }
        Ok(Self {
            path: Some(path),
            records,
        })
    }

    pub fn get(&self, name: &str) -> Option<&InstallRecord> {
        self.records.get(name)
    }

    pub fn records(&self) -> impl Iterator<Item = &InstallRecord> {
        self.records.values()
    }

    pub fn record(&mut self, record: InstallRecord) -> Result<(), InstallLedgerError> {
        self.records.insert(record.name.clone(), record);
        self.save()
    }

    pub fn remove(&mut self, name: &str) -> Result<bool, InstallLedgerError> {
        let existed = self.records.remove(name).is_some();
        if existed {
            self.save()?;
        }
        Ok(existed)
    }

    fn save(&self) -> Result<(), InstallLedgerError> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        let mut packages: Vec<&InstallRecord> = self.records.values().collect();
        packages.sort_by(|a, b| a.name.cmp(&b.name));
        let document = serde_json::json!({
            "version": LEDGER_VERSION,
            "packages": packages.into_iter().map(InstallRecord::to_json).collect::<Vec<_>>(),
        });
        let bytes =
            serde_json::to_vec_pretty(&document).map_err(|error| InstallLedgerError::Corrupt {
                reason: format!("ledger serialization failed: {error}"),
            })?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(InstallLedgerError::Io)?;
        }
        atomic_write_owner_only(path, &bytes).map_err(InstallLedgerError::Io)
    }
}

impl InstallRecord {
    fn from_json(value: &Value) -> Result<Self, InstallLedgerError> {
        let object = value
            .as_object()
            .ok_or_else(|| InstallLedgerError::Corrupt {
                reason: "install record must be an object".to_string(),
            })?;
        let required = |field: &str| -> Result<String, InstallLedgerError> {
            object
                .get(field)
                .and_then(Value::as_str)
                .map(str::to_string)
                .ok_or_else(|| InstallLedgerError::Corrupt {
                    reason: format!("install record missing `{field}`"),
                })
        };
        let pinned = object
            .get("pinned")
            .and_then(Value::as_bool)
            .ok_or_else(|| InstallLedgerError::Corrupt {
                reason: "install record missing `pinned`".to_string(),
            })?;
        Ok(Self {
            name: required("name")?,
            spec: required("spec")?,
            pinned,
            version: required("version")?,
            source: required("source")?,
            installed_at: required("installedAt")?,
        })
    }

    fn to_json(&self) -> Value {
        serde_json::json!({
            "name": self.name,
            "spec": self.spec,
            "pinned": self.pinned,
            "version": self.version,
            "source": self.source,
            "installedAt": self.installed_at,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "clay-ledger-test-{}-{}-{}",
            name,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock")
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn sample(name: &str, spec: &str, pinned: bool) -> InstallRecord {
        InstallRecord {
            name: name.to_string(),
            spec: spec.to_string(),
            pinned,
            version: "1.2.3".to_string(),
            source: "npm".to_string(),
            installed_at: "2026-09-09T10:00:00Z".to_string(),
        }
    }

    #[test]
    fn round_trips_and_removes() {
        let root = temp_root("roundtrip");
        let mut ledger = InstallLedger::open(&root).expect("open empty");
        ledger
            .record(sample("plain-mode", "npm:plain-mode", false))
            .unwrap();
        drop(ledger);

        let mut ledger = InstallLedger::open(&root).expect("reopen");
        let rec = ledger.get("plain-mode").expect("entry survived");
        assert!(!rec.pinned);
        assert_eq!(rec.spec, "npm:plain-mode");
        assert_eq!(rec.version, "1.2.3");
        assert_eq!(rec.source, "npm");
        assert!(ledger.remove("plain-mode").unwrap());
        drop(ledger);

        let ledger = InstallLedger::open(&root).expect("reopen after remove");
        assert!(ledger.get("plain-mode").is_none());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn pinned_flag_follows_exact_version_spec() {
        let root = temp_root("pinned");
        let mut ledger = InstallLedger::open(&root).unwrap();
        ledger
            .record(sample("@arnilo/st", "npm:@arnilo/st@1.2.3", true))
            .unwrap();
        ledger
            .record(sample("plain-mode", "npm:plain-mode", false))
            .unwrap();
        assert!(ledger.get("@arnilo/st").unwrap().pinned);
        assert!(!ledger.get("plain-mode").unwrap().pinned);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn fails_closed_on_corrupt_and_unknown_version() {
        let root = temp_root("corrupt");
        let path = root.join(INSTALL_LEDGER_FILE_NAME);
        fs::write(&path, b"{not json").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        }
        assert!(matches!(
            InstallLedger::open(&root),
            Err(InstallLedgerError::Corrupt { .. })
        ));

        fs::write(&path, b"{\"version\":99,\"packages\":[]}").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        }
        let error = InstallLedger::open(&root).unwrap_err();
        match error {
            InstallLedgerError::Corrupt { reason } => {
                assert!(reason.contains("unknown ledger version"));
            }
            other => panic!("expected corrupt, got {other:?}"),
        }
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn fails_closed_on_unsafe_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let root = temp_root("perms");
        let path = root.join(INSTALL_LEDGER_FILE_NAME);
        fs::write(&path, b"{\"version\":1,\"packages\":[]}").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(matches!(
            InstallLedger::open(&root),
            Err(InstallLedgerError::UnsafePermissions)
        ));
        let _ = fs::remove_dir_all(&root);
    }
}
