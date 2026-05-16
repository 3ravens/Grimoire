mod common;

use common::test_pool;

#[tokio::test]
async fn migrations_create_core_tables() {
    let pool = test_pool().await;

    let names: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    for required in [
        "audit_log",
        "bookmarks",
        "folders",
        "notes",
        "notes_fts",
        "property_defs",
        "settings",
        "tags",
        "templates",
        "vault_lock",
    ] {
        assert!(
            names.iter().any(|n| n == required),
            "missing table {required}, have: {names:?}"
        );
    }
}
