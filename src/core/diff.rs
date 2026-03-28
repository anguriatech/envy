//! Core diff logic — computes key-level differences between vault and artifact.
//!
//! All types and the [`compute_diff`] function are pure (no I/O, no crypto calls).
//! The CLI layer is responsible for rendering the [`DiffReport`].
//!
//! # Layer rules (Constitution Principle IV)
//! - MUST NOT import from `crate::cli`.
//! - No I/O, no filesystem, no network — pure computation only.

use std::collections::BTreeMap;

use zeroize::Zeroizing;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// The kind of change detected for a single secret key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeType {
    /// Key exists in vault but not in artifact.
    Added,
    /// Key exists in artifact but not in vault.
    Removed,
    /// Key exists in both but values differ.
    Modified,
}

/// A single key-level difference between vault and artifact.
#[derive(Debug)]
pub struct DiffEntry {
    pub key: String,
    pub change: ChangeType,
    /// Value from the artifact (`None` for Added entries).
    pub old_value: Option<Zeroizing<String>>,
    /// Value from the vault (`None` for Removed entries).
    pub new_value: Option<Zeroizing<String>>,
}

/// Complete diff result for one environment.
#[derive(Debug)]
pub struct DiffReport {
    pub env_name: String,
    pub entries: Vec<DiffEntry>,
    pub added: usize,
    pub removed: usize,
    pub modified: usize,
}

impl DiffReport {
    /// Returns `true` if there are any differences.
    pub fn has_differences(&self) -> bool {
        !self.entries.is_empty()
    }

    /// Total number of changes.
    pub fn total(&self) -> usize {
        self.added + self.removed + self.modified
    }
}

// ---------------------------------------------------------------------------
// compute_diff — stub (TDD: will be implemented after tests are written)
// ---------------------------------------------------------------------------

/// Compare vault secrets against artifact secrets for a single environment.
///
/// Both inputs are consumed (moved) to ensure `Zeroizing` values are properly
/// dropped after comparison. The returned [`DiffReport`] retains values only
/// for entries that differ — unchanged keys are dropped immediately.
///
/// # Arguments
/// - `env_name`: environment name (for the report header).
/// - `vault_secrets`: decrypted secrets from the local vault.
/// - `artifact_secrets`: decrypted secrets from the artifact envelope,
///   or an empty `BTreeMap` if the artifact is missing / env not in artifact.
///
/// # Returns
/// A [`DiffReport`] with entries sorted alphabetically by key.
pub fn compute_diff(
    env_name: &str,
    vault_secrets: BTreeMap<String, Zeroizing<String>>,
    artifact_secrets: BTreeMap<String, Zeroizing<String>>,
) -> DiffReport {
    let mut entries = Vec::new();
    let mut added: usize = 0;
    let mut removed: usize = 0;
    let mut modified: usize = 0;

    // Walk both sorted maps in tandem using iterators.
    let mut vault_iter = vault_secrets.into_iter().peekable();
    let mut artifact_iter = artifact_secrets.into_iter().peekable();

    loop {
        match (vault_iter.peek(), artifact_iter.peek()) {
            (Some((vk, _)), Some((ak, _))) => {
                match vk.cmp(ak) {
                    std::cmp::Ordering::Less => {
                        // Key in vault only → Added
                        let (key, value) = vault_iter.next().unwrap();
                        entries.push(DiffEntry {
                            key,
                            change: ChangeType::Added,
                            old_value: None,
                            new_value: Some(value),
                        });
                        added += 1;
                    }
                    std::cmp::Ordering::Greater => {
                        // Key in artifact only → Removed
                        let (key, value) = artifact_iter.next().unwrap();
                        entries.push(DiffEntry {
                            key,
                            change: ChangeType::Removed,
                            old_value: Some(value),
                            new_value: None,
                        });
                        removed += 1;
                    }
                    std::cmp::Ordering::Equal => {
                        // Key in both — compare values
                        let (key, new_val) = vault_iter.next().unwrap();
                        let (_, old_val) = artifact_iter.next().unwrap();
                        if *new_val != *old_val {
                            entries.push(DiffEntry {
                                key,
                                change: ChangeType::Modified,
                                old_value: Some(old_val),
                                new_value: Some(new_val),
                            });
                            modified += 1;
                        }
                        // Identical values → skip (dropped immediately)
                    }
                }
            }
            (Some(_), None) => {
                // Remaining vault keys → all Added
                let (key, value) = vault_iter.next().unwrap();
                entries.push(DiffEntry {
                    key,
                    change: ChangeType::Added,
                    old_value: None,
                    new_value: Some(value),
                });
                added += 1;
            }
            (None, Some(_)) => {
                // Remaining artifact keys → all Removed
                let (key, value) = artifact_iter.next().unwrap();
                entries.push(DiffEntry {
                    key,
                    change: ChangeType::Removed,
                    old_value: Some(value),
                    new_value: None,
                });
                removed += 1;
            }
            (None, None) => break,
        }
    }

    DiffReport {
        env_name: env_name.to_string(),
        entries,
        added,
        removed,
        modified,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: build a BTreeMap from key-value pairs.
    fn secrets(pairs: &[(&str, &str)]) -> BTreeMap<String, Zeroizing<String>> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), Zeroizing::new(v.to_string())))
            .collect()
    }

    // T002
    #[test]
    fn diff_all_added() {
        let vault = secrets(&[("A", "a"), ("B", "b")]);
        let artifact = BTreeMap::new();
        let report = compute_diff("development", vault, artifact);

        assert_eq!(report.entries.len(), 2, "expected 2 Added entries");
        assert!(report.has_differences());
        assert_eq!(report.added, 2);
        assert_eq!(report.removed, 0);
        assert_eq!(report.modified, 0);
        assert_eq!(report.entries[0].key, "A");
        assert_eq!(report.entries[1].key, "B");
        assert_eq!(report.entries[0].change, ChangeType::Added);
        assert_eq!(report.entries[1].change, ChangeType::Added);
    }

    // T003
    #[test]
    fn diff_all_removed() {
        let vault = BTreeMap::new();
        let artifact = secrets(&[("X", "x"), ("Y", "y")]);
        let report = compute_diff("development", vault, artifact);

        assert_eq!(report.entries.len(), 2, "expected 2 Removed entries");
        assert!(report.has_differences());
        assert_eq!(report.removed, 2);
        assert_eq!(report.entries[0].key, "X");
        assert_eq!(report.entries[1].key, "Y");
        assert_eq!(report.entries[0].change, ChangeType::Removed);
        assert_eq!(report.entries[1].change, ChangeType::Removed);
    }

    // T004
    #[test]
    fn diff_all_modified() {
        let vault = secrets(&[("A", "new")]);
        let artifact = secrets(&[("A", "old")]);
        let report = compute_diff("development", vault, artifact);

        assert_eq!(report.entries.len(), 1, "expected 1 Modified entry");
        assert!(report.has_differences());
        assert_eq!(report.modified, 1);
        assert_eq!(report.entries[0].change, ChangeType::Modified);
        assert_eq!(
            report.entries[0].old_value.as_ref().map(|v| v.as_str()),
            Some("old")
        );
        assert_eq!(
            report.entries[0].new_value.as_ref().map(|v| v.as_str()),
            Some("new")
        );
    }

    // T005
    #[test]
    fn diff_mixed_changes() {
        let vault = secrets(&[("A", "same"), ("B", "new_val"), ("D", "added")]);
        let artifact = secrets(&[("A", "same"), ("B", "old_val"), ("C", "removed")]);
        let report = compute_diff("development", vault, artifact);

        assert_eq!(report.entries.len(), 3, "A is identical, so 3 entries");
        assert_eq!(report.added, 1);
        assert_eq!(report.removed, 1);
        assert_eq!(report.modified, 1);
        // Sorted: B, C, D
        assert_eq!(report.entries[0].key, "B");
        assert_eq!(report.entries[0].change, ChangeType::Modified);
        assert_eq!(report.entries[1].key, "C");
        assert_eq!(report.entries[1].change, ChangeType::Removed);
        assert_eq!(report.entries[2].key, "D");
        assert_eq!(report.entries[2].change, ChangeType::Added);
    }

    // T006
    #[test]
    fn diff_no_changes() {
        let vault = secrets(&[("A", "v")]);
        let artifact = secrets(&[("A", "v")]);
        let report = compute_diff("development", vault, artifact);

        assert!(report.entries.is_empty(), "identical values => no entries");
        assert!(!report.has_differences());
        assert_eq!(report.total(), 0);
    }

    // T007
    #[test]
    fn diff_empty_both() {
        let report = compute_diff("development", BTreeMap::new(), BTreeMap::new());

        assert!(report.entries.is_empty());
        assert!(!report.has_differences());
    }

    // T008
    #[test]
    fn diff_sorted_output() {
        let vault = secrets(&[("Z", "z"), ("A", "a"), ("M", "m")]);
        let artifact = BTreeMap::new();
        let report = compute_diff("development", vault, artifact);

        assert_eq!(report.entries.len(), 3);
        assert_eq!(report.entries[0].key, "A");
        assert_eq!(report.entries[1].key, "M");
        assert_eq!(report.entries[2].key, "Z");
    }

    // T009
    #[test]
    fn diff_values_retained_for_modified() {
        let vault = secrets(&[("K", "new_secret")]);
        let artifact = secrets(&[("K", "old_secret")]);
        let report = compute_diff("development", vault, artifact);

        assert_eq!(report.entries.len(), 1);
        let entry = &report.entries[0];
        assert_eq!(entry.change, ChangeType::Modified);
        assert_eq!(
            entry.old_value.as_ref().map(|v| v.as_str()),
            Some("old_secret")
        );
        assert_eq!(
            entry.new_value.as_ref().map(|v| v.as_str()),
            Some("new_secret")
        );
    }
}
