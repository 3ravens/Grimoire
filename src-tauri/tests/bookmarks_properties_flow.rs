//! Scenario-style checks for bookmarks and property_defs (mirrors command SQL).

mod common;

use common::test_pool;

#[tokio::test]
async fn bookmarks_list_matches_join_query() {
    let pool = test_pool().await;

    let folder_id: i64 = sqlx::query_scalar("INSERT INTO folders (name) VALUES ('f') RETURNING id")
        .fetch_one(&pool)
        .await
        .unwrap();

    let note_id: i64 = sqlx::query_scalar(
        "INSERT INTO notes (title, content, folder_id) VALUES ('Marked', 'c', ?) RETURNING id",
    )
    .bind(folder_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    sqlx::query("INSERT OR IGNORE INTO bookmarks (note_id) VALUES (?)")
        .bind(note_id)
        .execute(&pool)
        .await
        .unwrap();

    let rows: Vec<(i64, String)> = sqlx::query_as(
        "SELECT b.note_id, n.title
         FROM bookmarks b
         JOIN notes n ON n.id = b.note_id
         ORDER BY b.added_at ASC",
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].0, note_id);
    assert_eq!(rows[0].1, "Marked");

    sqlx::query("DELETE FROM bookmarks WHERE note_id = ?")
        .bind(note_id)
        .execute(&pool)
        .await
        .unwrap();

    let cnt: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM bookmarks")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(cnt, 0);
}

#[tokio::test]
async fn property_defs_crud_round_trip() {
    let pool = test_pool().await;

    let folder_id: i64 = sqlx::query_scalar("INSERT INTO folders (name) VALUES ('db') RETURNING id")
        .fetch_one(&pool)
        .await
        .unwrap();

    let def_id: i64 = sqlx::query_scalar(
        "INSERT INTO property_defs (folder_id, name, type, options, position)
         VALUES (?, 'Status', 'text', NULL, 0) RETURNING id",
    )
    .bind(folder_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    let name: String = sqlx::query_scalar("SELECT name FROM property_defs WHERE id = ?")
        .bind(def_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(name, "Status");

    sqlx::query("UPDATE property_defs SET name = 'State' WHERE id = ?")
        .bind(def_id)
        .execute(&pool)
        .await
        .unwrap();

    let name2: String = sqlx::query_scalar("SELECT name FROM property_defs WHERE id = ?")
        .bind(def_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(name2, "State");

    sqlx::query("DELETE FROM property_defs WHERE id = ?")
        .bind(def_id)
        .execute(&pool)
        .await
        .unwrap();
}

#[tokio::test]
async fn user_template_insert_and_select() {
    let pool = test_pool().await;

    let id: i64 = sqlx::query_scalar(
        "INSERT INTO templates (name, title, content, properties)
         VALUES ('MyTpl', 'Seed', 'Hello', '[]') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let (name, title): (String, String) = sqlx::query_as("SELECT name, title FROM templates WHERE id = ?")
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(name, "MyTpl");
    assert_eq!(title, "Seed");
}
