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

    /// Number of days a secret can go without being updated before `envy status`
    /// flags it as due for rotation. Absent in manifests written before this
    /// field existed — `serde(default)` keeps those files parseable unchanged.
    #[serde(default = "default_rotation_reminder_days")]
    pub rotation_reminder_days: u32,

    /// Opt-in flag for transparent shell auto-injection (`envy hook` /
    /// `envy shell-init`). When `false` (default, including all manifests
    /// written before this field existed), `envy hook` is a silent no-op that
    /// only unloads previously exported keys. Enable with `envy auto on` or
    /// `envy init --auto-inject`.
    #[serde(default)]
    pub auto_inject: bool,
}

/// Default rotation reminder threshold (90 days) used when `envy.toml` omits
/// the field, or when the field is missing from manifests created by older
/// `envy` versions.
fn default_rotation_reminder_days() -> u32 {
    90
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
    create_manifest_with_options(target_dir, project_id, false)
}

/// Creates `envy.toml` in `target_dir` with an explicit `auto_inject` flag.
///
/// Same contract as [`create_manifest`], plus the transparent auto-injection
/// opt-in used by `envy hook` / `envy shell-init`.
pub fn create_manifest_with_options(
    target_dir: &Path,
    project_id: &str,
    auto_inject: bool,
) -> Result<(), CoreError> {
    use std::io::Write as _;

    let path = target_dir.join("envy.toml");
    // Hand-format the TOML so we can prepend the human-readable comment header.
    // The project_id is always a UUID (alphanumeric + hyphens), so no TOML
    // escaping is required.
    let content = format!(
        "# Created by `envy init`. Do not delete — this file links the directory to its vault.\nproject_id = \"{}\"\n\n# Transparent shell auto-injection (`eval \"$(envy shell-init <shell>)\"` + `envy hook`).\n# When true, entering this directory auto-exports vault secrets into the shell,\n# taking precedence over a legacy `.env` file (a warning is printed).\n# Manage with `envy auto on|off|status`. Default: false.\nauto_inject = {}\n\n# Days a secret can go unmodified before `envy status` flags it for rotation.\n# Uncomment to override the default (90).\n# rotation_reminder_days = 90\n",
        project_id, auto_inject
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

/// Enables or disables transparent auto-injection for an existing project.
///
/// `manifest_dir` is the **directory** returned by [`find_manifest`] (not the
/// file path). The edit is text-based — an existing `auto_inject` line is
/// replaced, otherwise the flag is inserted after the `project_id` line — so
/// unrelated settings (e.g. a custom `rotation_reminder_days`) and comments
/// are preserved.
///
/// # Errors
/// - [`CoreError::ManifestIo`] if the file cannot be read or written.
/// - [`CoreError::ManifestInvalid`] if the file parses but the write-back fails
///   (structurally impossible; surfaced for completeness).
pub fn set_manifest_auto_inject(manifest_dir: &Path, auto_inject: bool) -> Result<(), CoreError> {
    let path = manifest_dir.join("envy.toml");
    let content =
        std::fs::read_to_string(&path).map_err(|e| CoreError::ManifestIo(e.to_string()))?;
    // Validate before editing so a corrupt manifest is reported, not rewritten.
    let _: Manifest =
        toml::from_str(&content).map_err(|e| CoreError::ManifestInvalid(e.to_string()))?;

    let replacement = format!("auto_inject = {auto_inject}");
    let mut out: Vec<String> = Vec::new();
    let mut replaced = false;
    for line in content.lines() {
        let trimmed = line.trim_start();
        let is_flag_line = trimmed
            .strip_prefix("auto_inject")
            .map(|rest| rest.trim_start().starts_with('='))
            .unwrap_or(false);
        if is_flag_line && !replaced {
            // Preserve leading indentation (normally none).
            let indent_len = line.len() - trimmed.len();
            out.push(format!("{}{replacement}", &line[..indent_len]));
            replaced = true;
        } else {
            out.push(line.to_string());
        }
    }
    if !replaced {
        // Insert after the project_id line so the flag sits with project identity.
        let mut inserted = false;
        let mut with_insert: Vec<String> = Vec::with_capacity(out.len() + 1);
        for line in out {
            with_insert.push(line.clone());
            if !inserted && line.trim_start().starts_with("project_id") && line.contains('=') {
                with_insert.push(replacement.clone());
                inserted = true;
            }
        }
        if !inserted {
            with_insert.push(replacement.clone());
        }
        out = with_insert;
    }
    let mut new_content = out.join("\n");
    new_content.push('\n');
    std::fs::write(&path, new_content).map_err(|e| CoreError::ManifestIo(e.to_string()))?;
    Ok(())
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
