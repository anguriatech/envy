// Integration test entry point for the database layer.
// Each #[path] attribute explicitly points into the tests/db/ subdirectory,
// since this file is a crate root and Rust would otherwise look for siblings
// at tests/*.rs rather than tests/db/*.rs.

#[path = "db/test_schema.rs"]
mod test_schema;

#[path = "db/test_projects.rs"]
mod test_projects;

#[path = "db/test_environments.rs"]
mod test_environments;

#[path = "db/test_secrets.rs"]
mod test_secrets;

#[path = "db/test_security.rs"]
mod test_security;
