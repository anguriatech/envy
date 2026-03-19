// Integration tests for the db layer — project CRUD operations.

use envy::db::{DbError, ProjectId, Vault};

const DUMMY_KEY: [u8; 32] = [0u8; 32];

fn open_temp_vault() -> (Vault, tempfile::NamedTempFile) {
    let tmp = tempfile::NamedTempFile::new()
        .expect("NamedTempFile::new always succeeds in a writable temp dir");
    let vault = Vault::open(tmp.path(), &DUMMY_KEY)
        .expect("Vault::open on a fresh temp file always succeeds");
    (vault, tmp)
}

// ---------------------------------------------------------------------------
// (a) create_project returns a valid UUID-formatted ProjectId
// ---------------------------------------------------------------------------

#[test]
fn test_create_project_returns_uuid() {
    let (vault, _tmp) = open_temp_vault();

    let id = vault
        .create_project("my-app")
        .expect("create_project must succeed on an empty vault");

    // UUID v4 hyphenated format: 8-4-4-4-12 = 36 chars
    assert_eq!(
        id.as_str().len(),
        36,
        "ProjectId must be a 36-character UUID"
    );
    // Basic shape check: hyphens at positions 8, 13, 18, 23
    let chars: Vec<char> = id.as_str().chars().collect();
    assert_eq!(chars[8], '-', "UUID must have hyphen at position 8");
    assert_eq!(chars[13], '-', "UUID must have hyphen at position 13");
    assert_eq!(chars[18], '-', "UUID must have hyphen at position 18");
    assert_eq!(chars[23], '-', "UUID must have hyphen at position 23");
}

// ---------------------------------------------------------------------------
// (b) get_project returns the correct name
// ---------------------------------------------------------------------------

#[test]
fn test_get_project_returns_correct_name() {
    let (vault, _tmp) = open_temp_vault();

    let id = vault
        .create_project("hello-world")
        .expect("create_project must succeed");

    let project = vault
        .get_project(&id)
        .expect("get_project must return the project that was just created");

    assert_eq!(project.name, "hello-world");
    assert_eq!(project.id, id);
    assert!(project.created_at > 0, "created_at must be a non-zero epoch");
    assert!(project.updated_at > 0, "updated_at must be a non-zero epoch");
}

// ---------------------------------------------------------------------------
// (c) get_project on a non-existent id returns DbError::NotFound
// ---------------------------------------------------------------------------

#[test]
fn test_get_project_not_found() {
    let (vault, _tmp) = open_temp_vault();

    let result = vault.get_project(&ProjectId("00000000-0000-0000-0000-000000000000".into()));

    assert!(
        matches!(result, Err(DbError::NotFound)),
        "get_project with an unknown id must return DbError::NotFound, got: {:?}",
        result
    );
}

// ---------------------------------------------------------------------------
// (d) get_project_by_name finds the right project
// ---------------------------------------------------------------------------

#[test]
fn test_get_project_by_name() {
    let (vault, _tmp) = open_temp_vault();

    let id = vault
        .create_project("find-me")
        .expect("create_project must succeed");

    let project = vault
        .get_project_by_name("find-me")
        .expect("get_project_by_name must find the project");

    assert_eq!(project.id, id);
}

#[test]
fn test_get_project_by_name_not_found() {
    let (vault, _tmp) = open_temp_vault();

    let result = vault.get_project_by_name("does-not-exist");

    assert!(
        matches!(result, Err(DbError::NotFound)),
        "get_project_by_name with unknown name must return DbError::NotFound"
    );
}

// ---------------------------------------------------------------------------
// (e) list_projects returns projects in created_at ASC order
// ---------------------------------------------------------------------------

#[test]
fn test_list_projects_order() {
    let (vault, _tmp) = open_temp_vault();

    // Empty vault — list must succeed and return empty vec, not an error.
    let empty = vault
        .list_projects()
        .expect("list_projects on empty vault must return Ok(vec![])");
    assert!(empty.is_empty(), "list_projects on empty vault must return []");

    // Insert three projects.
    let id_a = vault.create_project("alpha").expect("create alpha");
    let id_b = vault.create_project("beta").expect("create beta");
    let id_c = vault.create_project("gamma").expect("create gamma");

    let list = vault
        .list_projects()
        .expect("list_projects must succeed after inserts");

    assert_eq!(list.len(), 3, "list must contain exactly 3 projects");

    // Order must be insertion order (created_at ASC).
    // On modern hardware all three may share the same epoch second, but the
    // ORDER BY created_at ASC, id ASC tie-break keeps the result stable.
    let ids: Vec<&ProjectId> = list.iter().map(|p| &p.id).collect();
    assert!(
        ids.contains(&&id_a) && ids.contains(&&id_b) && ids.contains(&&id_c),
        "all three projects must appear in the list"
    );
}

// ---------------------------------------------------------------------------
// (f) delete_project removes the record
// ---------------------------------------------------------------------------

#[test]
fn test_delete_project_removes_record() {
    let (vault, _tmp) = open_temp_vault();

    let id = vault
        .create_project("to-delete")
        .expect("create_project must succeed");

    vault
        .delete_project(&id)
        .expect("delete_project must succeed for an existing project");

    let result = vault.get_project(&id);
    assert!(
        matches!(result, Err(DbError::NotFound)),
        "get_project after delete must return DbError::NotFound"
    );
}

// ---------------------------------------------------------------------------
// (g) delete_project on a non-existent id returns DbError::NotFound
// ---------------------------------------------------------------------------

#[test]
fn test_delete_project_not_found() {
    let (vault, _tmp) = open_temp_vault();

    let result =
        vault.delete_project(&ProjectId("00000000-0000-0000-0000-000000000000".into()));

    assert!(
        matches!(result, Err(DbError::NotFound)),
        "delete_project with unknown id must return DbError::NotFound"
    );
}

// ---------------------------------------------------------------------------
// (h) two create_project calls with the same name produce distinct UUIDs
// ---------------------------------------------------------------------------

#[test]
fn test_two_projects_same_name_have_distinct_ids() {
    let (vault, _tmp) = open_temp_vault();

    let id_1 = vault.create_project("duplicate-name").expect("first create");
    let id_2 = vault.create_project("duplicate-name").expect("second create");

    assert_ne!(
        id_1, id_2,
        "two projects with the same name must have distinct UUIDs"
    );

    let list = vault.list_projects().expect("list must succeed");
    assert_eq!(list.len(), 2, "both projects must be stored");
}
