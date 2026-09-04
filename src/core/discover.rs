//! Downward `envy.toml` discovery for the interactive TUI (FR-061).
//!
//! [`discover_projects`] walks the directory tree **downward** from a launch
//! directory (unbounded depth, `.gitignore` respected) collecting every
//! project manifest at or below it. This scopes the TUI sidebar to the
//! *workspace* — the projects the user can actually reach from where they
//! launched `envy` — instead of the whole vault, which lets every sidebar
//! project carry its own correct artifact path (FR-022).

use std::path::{Path, PathBuf};

use super::Manifest;

/// A project manifest found at or below the launch directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredProject {
    /// The UUID v4 linking this directory to its vault entry.
    pub project_id: String,

    /// Per-project rotation reminder threshold parsed from the manifest.
    pub rotation_reminder_days: u32,

    /// The directory containing `envy.toml` (the project root).
    pub manifest_dir: PathBuf,

    /// The sealed artifact beside the manifest (`manifest_dir/envy.enc`).
    pub artifact_path: PathBuf,
}

/// Collects every `envy.toml` at or below `root`, parsed.
///
/// The walk respects `.gitignore`/`.ignore` files (even outside a real git
/// repository — see [`crate::core::scan`]) and skips hidden directories such
/// as `.git`. `root` itself is checked first, so launching inside a project
/// directory includes it. Malformed manifests (invalid TOML, missing
/// `project_id`) are skipped rather than aborting the whole discovery — a
/// half-written manifest must not block the session.
///
/// Results are sorted by manifest directory for a stable sidebar order. If
/// the same `project_id` appears in two directories (a copied manifest), the
/// entry earliest in that stable order wins and later duplicates are dropped
/// — the choice never depends on filesystem walk order.
pub fn discover_projects(root: &Path) -> Vec<DiscoveredProject> {
    let mut found: Vec<DiscoveredProject> = Vec::new();
    if let Some(project) = parse_manifest_dir(root.to_path_buf()) {
        found.push(project);
    }

    let walker = ignore::WalkBuilder::new(root)
        .hidden(true)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        // `.gitignore` must be honoured even when `root` isn't (yet) inside
        // an actual `.git` repository — envy projects are frequently scanned
        // before the first `git init`.
        .require_git(false)
        .build();

    for entry in walker.flatten() {
        if entry.file_type().is_some_and(|t| t.is_dir())
            && let Some(project) = parse_manifest_dir(entry.into_path())
        {
            found.push(project);
        }
    }

    found.sort_by(|a, b| a.manifest_dir.cmp(&b.manifest_dir));
    // dedup_by keeps the first of each equal run — after the sort that is
    // the lexicographically-first manifest directory for that id.
    found.dedup_by(|a, b| a.project_id == b.project_id);
    found
}

/// Parses `dir/envy.toml`, returning `None` when the file does not exist or
/// cannot be parsed (any other I/O error included — a unreadable manifest is
/// treated the same as an absent one).
fn parse_manifest_dir(dir: PathBuf) -> Option<DiscoveredProject> {
    let content = std::fs::read_to_string(dir.join("envy.toml")).ok()?;
    let manifest: Manifest = toml::from_str(&content).ok()?;
    Some(DiscoveredProject {
        project_id: manifest.project_id,
        rotation_reminder_days: manifest.rotation_reminder_days,
        artifact_path: dir.join("envy.enc"),
        manifest_dir: dir,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn write_manifest(dir: &Path, project_id: &str) {
        std::fs::write(
            dir.join("envy.toml"),
            format!("project_id = \"{project_id}\"\n"),
        )
        .expect("write envy.toml");
    }

    #[test]
    fn discovers_root_and_nested_manifests() {
        let temp = tempfile::tempdir().expect("tempdir");
        write_manifest(temp.path(), "root");
        let child = temp.path().join("super-envy");
        let grandchild = child.join("sub").join("deeper");
        std::fs::create_dir_all(&grandchild).expect("create dirs");
        write_manifest(&child, "child");
        write_manifest(&grandchild, "grandchild");

        let out = discover_projects(temp.path());
        let ids: Vec<&str> = out.iter().map(|p| p.project_id.as_str()).collect();
        assert_eq!(ids, vec!["root", "child", "grandchild"]);
        // Artifact path sits beside each manifest.
        assert_eq!(out[0].artifact_path, temp.path().join("envy.enc"));
        assert_eq!(out[2].artifact_path, grandchild.join("envy.enc"));
    }

    #[test]
    fn empty_root_returns_nothing() {
        let temp = tempfile::tempdir().expect("tempdir");
        assert!(discover_projects(temp.path()).is_empty());
    }

    #[test]
    fn gitignored_manifest_is_excluded() {
        let temp = tempfile::tempdir().expect("tempdir");
        let ignored = temp.path().join("ignored");
        std::fs::create_dir(&ignored).expect("create dir");
        write_manifest(&ignored, "ignored");
        std::fs::write(temp.path().join(".gitignore"), "/ignored/\n").expect("gitignore");
        write_manifest(temp.path(), "root");

        let out = discover_projects(temp.path());
        let ids: Vec<&str> = out.iter().map(|p| p.project_id.as_str()).collect();
        assert_eq!(ids, vec!["root"]);
    }

    #[test]
    fn hidden_directories_are_not_walked() {
        let temp = tempfile::tempdir().expect("tempdir");
        let dot = temp.path().join(".cache");
        std::fs::create_dir(&dot).expect("create dir");
        write_manifest(&dot, "hidden");

        assert!(discover_projects(temp.path()).is_empty());
    }

    #[test]
    fn malformed_manifest_is_skipped() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(temp.path().join("envy.toml"), "not [valid toml").expect("bad toml");
        let child = temp.path().join("child");
        std::fs::create_dir(&child).expect("create dir");
        write_manifest(&child, "child");

        let out = discover_projects(temp.path());
        let ids: Vec<&str> = out.iter().map(|p| p.project_id.as_str()).collect();
        assert_eq!(ids, vec!["child"]);
    }

    #[test]
    fn duplicate_project_id_keeps_stable_order_winner() {
        let temp = tempfile::tempdir().expect("tempdir");
        let a = temp.path().join("a");
        let b = temp.path().join("b");
        std::fs::create_dir_all(&a).expect("create dir");
        std::fs::create_dir_all(&b).expect("create dir");
        write_manifest(&a, "same");
        write_manifest(&b, "same");

        let out = discover_projects(temp.path());
        assert_eq!(out.len(), 1, "same project_id must collapse to one entry");
        assert_eq!(out[0].manifest_dir, a, "sorted order: 'a' precedes 'b'");
    }
}
