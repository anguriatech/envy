// Integration tests for the db layer — environment CRUD, uniqueness, cascade deletes.

use envy::db::{DbError, EnvId, ProjectId, Vault};

const DUMMY_KEY: [u8; 32] = [0u8; 32];

fn open_temp_vault() -> (Vault, tempfile::NamedTempFile) {
    let tmp = tempfile::NamedTempFile::new()
        .expect("NamedTempFile::new always succeeds in a writable temp dir");
    let vault = Vault::open(tmp.path(), &DUMMY_KEY)
        .expect("Vault::open on a fresh temp file always succeeds");
    (vault, tmp)
}

/// Creates a project and returns its id. Used as setup in environment tests.
fn make_project(vault: &Vault, name: &str) -> ProjectId {
    vault
        .create_project(name)
        .expect("create_project must succeed in setup")
}

// ---------------------------------------------------------------------------
// (a) create_environment succeeds with a lowercase name
// ---------------------------------------------------------------------------

#[test]
fn test_create_environment_succeeds() {
    let (vault, _tmp) = open_temp_vault();
    let project_id = make_project(&vault, "my-app");

    let env_id = vault
        .create_environment(&project_id, "development")
        .expect("create_environment must succeed with a valid lowercase name");

    // UUID v4 hyphenated format: 36 chars
    assert_eq!(
        env_id.as_str().len(),
        36,
        "EnvId must be a 36-character UUID"
    );
}

// ---------------------------------------------------------------------------
// (b) duplicate (project_id, name) returns DbError::AlreadyExists
// ---------------------------------------------------------------------------

#[test]
fn test_create_environment_duplicate_returns_already_exists() {
    let (vault, _tmp) = open_temp_vault();
    let project_id = make_project(&vault, "my-app");

    vault
        .create_environment(&project_id, "production")
        .expect("first create_environment must succeed");

    let result = vault.create_environment(&project_id, "production");

    assert!(
        matches!(result, Err(DbError::AlreadyExists)),
        "duplicate environment name must return DbError::AlreadyExists, got: {:?}",
        result
    );
}

// ---------------------------------------------------------------------------
// (c) get_environment_by_name returns the correct record
// ---------------------------------------------------------------------------

#[test]
fn test_get_environment_by_name() {
    let (vault, _tmp) = open_temp_vault();
    let project_id = make_project(&vault, "my-app");

    let env_id = vault
        .create_environment(&project_id, "staging")
        .expect("create_environment must succeed");

    let env = vault
        .get_environment_by_name(&project_id, "staging")
        .expect("get_environment_by_name must find the environment");

    assert_eq!(env.id, env_id);
    assert_eq!(env.name, "staging");
    assert_eq!(env.project_id, project_id);
    assert!(env.created_at > 0, "created_at must be a non-zero epoch");
    assert!(env.updated_at > 0, "updated_at must be a non-zero epoch");
}

#[test]
fn test_get_environment_by_name_not_found() {
    let (vault, _tmp) = open_temp_vault();
    let project_id = make_project(&vault, "my-app");

    let result = vault.get_environment_by_name(&project_id, "does-not-exist");

    assert!(
        matches!(result, Err(DbError::NotFound)),
        "get_environment_by_name with unknown name must return DbError::NotFound"
    );
}

// ---------------------------------------------------------------------------
// (d) non-existent project_id returns DbError::ConstraintViolation (FK)
// ---------------------------------------------------------------------------

#[test]
fn test_create_environment_invalid_project_id() {
    let (vault, _tmp) = open_temp_vault();
    let ghost_project = ProjectId("00000000-0000-0000-0000-000000000000".into());

    let result = vault.create_environment(&ghost_project, "development");

    assert!(
        matches!(result, Err(DbError::ConstraintViolation(_))),
        "create_environment with non-existent project_id must return DbError::ConstraintViolation, got: {:?}",
        result
    );
}

// ---------------------------------------------------------------------------
// (e) delete_project cascades to environments
// ---------------------------------------------------------------------------

#[test]
fn test_delete_project_cascades_to_environments() {
    let (vault, _tmp) = open_temp_vault();
    let project_id = make_project(&vault, "my-app");

    let env_id = vault
        .create_environment(&project_id, "development")
        .expect("create_environment must succeed");

    vault
        .delete_project(&project_id)
        .expect("delete_project must succeed");

    // The environment must be gone via ON DELETE CASCADE
    let result = vault.get_environment(&env_id);
    assert!(
        matches!(result, Err(DbError::NotFound)),
        "get_environment after project delete must return DbError::NotFound"
    );
}

// ---------------------------------------------------------------------------
// (f) CHECK(name = lower(name)) is enforced at the DB level
// ---------------------------------------------------------------------------

#[test]
fn test_uppercase_environment_name_is_rejected() {
    let (vault, _tmp) = open_temp_vault();
    let project_id = make_project(&vault, "my-app");

    // The caller contract says names must be pre-lowercased, but the schema's
    // CHECK(name = lower(name)) is the second line of defense. Verify it fires.
    let result = vault.create_environment(&project_id, "Production");

    assert!(
        matches!(result, Err(DbError::ConstraintViolation(_))),
        "uppercase environment name must return DbError::ConstraintViolation, got: {:?}",
        result
    );
}

// ---------------------------------------------------------------------------
// (g) list_environments returns records ordered by name ASC
// ---------------------------------------------------------------------------

#[test]
fn test_list_environments_order() {
    let (vault, _tmp) = open_temp_vault();
    let project_id = make_project(&vault, "my-app");

    // Empty project — list must return Ok(vec![])
    let empty = vault
        .list_environments(&project_id)
        .expect("list_environments on empty project must return Ok(vec![])");
    assert!(
        empty.is_empty(),
        "list_environments on empty project must return []"
    );

    vault
        .create_environment(&project_id, "staging")
        .expect("create staging");
    vault
        .create_environment(&project_id, "development")
        .expect("create development");
    vault
        .create_environment(&project_id, "production")
        .expect("create production");

    let list = vault
        .list_environments(&project_id)
        .expect("list_environments must succeed after inserts");

    assert_eq!(list.len(), 3, "list must contain exactly 3 environments");

    let names: Vec<&str> = list.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(
        names,
        vec!["development", "production", "staging"],
        "environments must be ordered by name ASC"
    );
}

// ---------------------------------------------------------------------------
// (h) delete_environment removes the record; NotFound on second delete
// ---------------------------------------------------------------------------

#[test]
fn test_delete_environment_removes_record() {
    let (vault, _tmp) = open_temp_vault();
    let project_id = make_project(&vault, "my-app");

    let env_id = vault
        .create_environment(&project_id, "to-delete")
        .expect("create_environment must succeed");

    vault
        .delete_environment(&env_id)
        .expect("delete_environment must succeed for an existing environment");

    let result = vault.get_environment(&env_id);
    assert!(
        matches!(result, Err(DbError::NotFound)),
        "get_environment after delete must return DbError::NotFound"
    );
}

#[test]
fn test_delete_environment_not_found() {
    let (vault, _tmp) = open_temp_vault();

    let result = vault.delete_environment(&EnvId("00000000-0000-0000-0000-000000000000".into()));

    assert!(
        matches!(result, Err(DbError::NotFound)),
        "delete_environment with unknown id must return DbError::NotFound"
    );
}

// ---------------------------------------------------------------------------
// (i) same name in different projects is allowed
// ---------------------------------------------------------------------------

#[test]
fn test_same_env_name_in_different_projects() {
    let (vault, _tmp) = open_temp_vault();
    let project_a = make_project(&vault, "project-a");
    let project_b = make_project(&vault, "project-b");

    vault
        .create_environment(&project_a, "production")
        .expect("create production in project-a must succeed");

    // Same name in a different project — must NOT return AlreadyExists
    vault
        .create_environment(&project_b, "production")
        .expect("create production in project-b must succeed");
}
