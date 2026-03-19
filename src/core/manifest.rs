//! `envy.toml` discovery and creation.
//!
//! [`find_manifest`] walks the directory tree upward from a given starting
//! directory until it finds `envy.toml`, then parses and returns it.
//! [`create_manifest`] creates a fresh `envy.toml` in a given directory.

use std::path::{Path, PathBuf};

use super::CoreError;

// ---------------------------------------------------------------------------
// T009 — Manifest struct
// ---------------------------------------------------------------------------

/// The parsed contents of an `envy.toml` project manifest.
///
/// Serialised/deserialised via `serde` + the `toml` crate. Additional fields
/// may be added in future features without breaking this struct (serde ignores
/// unknown fields by default).
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Manifest {
    /// The UUID v4 that links this directory tree to its vault entry.
    pub project_id: String,
}

// ---------------------------------------------------------------------------
// T010 — find_manifest
// ---------------------------------------------------------------------------

/// Walks upward from `start_dir` searching for `envy.toml`.
///
/// Returns the parsed manifest and the **directory** it was found in (not the
/// file path). This lets callers resolve sibling files relative to the manifest.
///
/// # Errors
/// - [`CoreError::ManifestNotFound`] if no `envy.toml` exists between
///   `start_dir` and the filesystem root.
/// - [`CoreError::ManifestInvalid`] if a file is found but fails TOML parsing
///   or is missing the required `project_id` field.
/// - [`CoreError::ManifestIo`] if the file cannot be read (permissions, etc.).
pub fn find_manifest(start_dir: &Path) -> Result<(Manifest, PathBuf), CoreError> {
    let mut dir = start_dir.to_path_buf();
    loop {
        let candidate = dir.join("envy.toml");
        match std::fs::read_to_string(&candidate) {
            Ok(content) => {
                let manifest = toml::from_str::<Manifest>(&content)
                    .map_err(|e| CoreError::ManifestInvalid(e.to_string()))?;
                return Ok((manifest, dir));
            }
            // File does not exist at this level — move up.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            // Unexpected I/O error (permissions, broken symlink, etc.).
            Err(e) => return Err(CoreError::ManifestIo(e.to_string())),
        }
        match dir.parent() {
            Some(parent) => dir = parent.to_path_buf(),
            // Reached the filesystem root with no manifest found.
            None => return Err(CoreError::ManifestNotFound),
        }
    }
}

// ---------------------------------------------------------------------------
// T011 — create_manifest
// ---------------------------------------------------------------------------

/// Creates `envy.toml` in `target_dir` containing the given `project_id`.
///
/// Fails if the file already exists or cannot be written (returns
/// [`CoreError::ManifestIo`] for both cases so the caller does not need to
/// distinguish them).
///
/// # Errors
/// - [`CoreError::ManifestIo`] if the file already exists, the directory is
///   not writable, or any other I/O failure occurs.
pub fn create_manifest(target_dir: &Path, project_id: &str) -> Result<(), CoreError> {
    use std::io::Write as _;

    let path = target_dir.join("envy.toml");
    // Hand-format the TOML so we can prepend the human-readable comment header.
    // The project_id is always a UUID (alphanumeric + hyphens), so no TOML
    // escaping is required.
    let content = format!(
        "# Created by `envy init`. Do not delete — this file links the directory to its vault.\nproject_id = \"{}\"\n",
        project_id
    );
    // `create_new(true)` fails with AlreadyExists if the file exists,
    // satisfying the "do not silently overwrite" invariant.
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|e| CoreError::ManifestIo(e.to_string()))?;
    file.write_all(content.as_bytes())
        .map_err(|e| CoreError::ManifestIo(e.to_string()))
}

// ---------------------------------------------------------------------------
// T005–T008 — Tests (written first to define the contract; failed before T009–T011)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // T005
    #[test]
    fn find_manifest_in_current_dir() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let project_id = "550e8400-e29b-41d4-a716-446655440000";
        std::fs::write(
            tmp.path().join("envy.toml"),
            format!("project_id = \"{}\"\n", project_id),
        )
        .expect("write envy.toml");

        let (manifest, found_dir) = find_manifest(tmp.path()).expect("find_manifest must succeed");
        assert_eq!(manifest.project_id, project_id);
        assert_eq!(found_dir, tmp.path());
    }

    // T006
    #[test]
    fn find_manifest_in_parent_dir() {
        let parent = tempfile::tempdir().expect("tempdir");
        let grandchild = parent.path().join("child").join("grandchild");
        std::fs::create_dir_all(&grandchild).expect("create subdirectories");

        let project_id = "6ba7b810-9dad-11d1-80b4-00c04fd430c8";
        std::fs::write(
            parent.path().join("envy.toml"),
            format!("project_id = \"{}\"\n", project_id),
        )
        .expect("write envy.toml");

        let (manifest, found_dir) = find_manifest(&grandchild).expect("find_manifest must succeed");
        assert_eq!(manifest.project_id, project_id);
        assert_eq!(found_dir, parent.path());
    }

    // T007
    #[test]
    fn find_manifest_not_found() {
        // A fresh temp dir in /tmp has no envy.toml in any ancestor up to /.
        let tmp = tempfile::tempdir().expect("tempdir");
        let result = find_manifest(tmp.path());
        assert!(
            matches!(result, Err(CoreError::ManifestNotFound)),
            "expected ManifestNotFound, got: {:?}",
            result
        );
    }

    // T008
    #[test]
    fn create_and_read_manifest() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let project_id = "f47ac10b-58cc-4372-a567-0e02b2c3d479";

        create_manifest(tmp.path(), project_id).expect("create_manifest must succeed");

        // (1) File exists.
        assert!(
            tmp.path().join("envy.toml").exists(),
            "envy.toml must be created"
        );

        // (2) project_id field round-trips through TOML.
        let content = std::fs::read_to_string(tmp.path().join("envy.toml")).expect("read file");
        let parsed: Manifest = toml::from_str(&content).expect("parse TOML");
        assert_eq!(parsed.project_id, project_id);

        // (3) find_manifest returns Ok with matching project_id.
        let (found, _dir) = find_manifest(tmp.path()).expect("find_manifest must succeed");
        assert_eq!(found.project_id, project_id);
    }
}
