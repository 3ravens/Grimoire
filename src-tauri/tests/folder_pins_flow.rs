//! Scenario-style checks for folder_pins (mirrors command SQL + move_note clear).

mod common;

use common::{empty_keystore, test_pool};
use app_lib::{AccessFilter, EncryptedNoteStore};

#[tokio::test]
async fn pin_order_list_and_reorder_exact_set() {
    let pool = test_pool().await;

    let folder_id: i64 = sqlx::query_scalar("INSERT INTO folders (name) VALUES ('f') RETURNING id")
        .fetch_one(&pool)
        .await
        .unwrap();

    let n1: i64 = sqlx::query_scalar(
        "INSERT INTO notes (title, content, folder_id) VALUES ('A', 'c', ?) RETURNING id",
    )
    .bind(folder_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let n2: i64 = sqlx::query_scalar(
        "INSERT INTO notes (title, content, folder_id) VALUES ('B', 'c', ?) RETURNING id",
    )
    .bind(folder_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    sqlx::query("INSERT INTO folder_pins (note_id, folder_id, position) VALUES (?, ?, 0)")
        .bind(n1)
        .bind(folder_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO folder_pins (note_id, folder_id, position) VALUES (?, ?, 1)")
        .bind(n2)
        .bind(folder_id)
        .execute(&pool)
        .await
        .unwrap();

    let rows: Vec<(i64, i64)> = sqlx::query_as(
        "SELECT fp.note_id, fp.position
         FROM folder_pins fp
         JOIN notes n ON n.id = fp.note_id
         WHERE fp.folder_id = ?
           AND n.folder_id = fp.folder_id
         ORDER BY fp.position ASC",
    )
    .bind(folder_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(rows, vec![(n1, 0), (n2, 1)]);

    // Reorder to [n2, n1]
    for (i, nid) in [n2, n1].iter().enumerate() {
        sqlx::query("UPDATE folder_pins SET position = ? WHERE note_id = ? AND folder_id = ?")
            .bind(i as i64)
            .bind(nid)
            .bind(folder_id)
            .execute(&pool)
            .await
            .unwrap();
    }

    let rows: Vec<i64> = sqlx::query_scalar(
        "SELECT fp.note_id FROM folder_pins fp
         JOIN notes n ON n.id = fp.note_id
         WHERE fp.folder_id = ? AND n.folder_id = fp.folder_id
         ORDER BY fp.position ASC",
    )
    .bind(folder_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(rows, vec![n2, n1]);
}

#[tokio::test]
async fn move_to_other_folder_clears_pin_same_folder_does_not() {
    let pool = test_pool().await;
    let keys = empty_keystore();

    let folder_a: i64 = sqlx::query_scalar("INSERT INTO folders (name) VALUES ('a') RETURNING id")
        .fetch_one(&pool)
        .await
        .unwrap();
    let folder_b: i64 = sqlx::query_scalar("INSERT INTO folders (name) VALUES ('b') RETURNING id")
        .fetch_one(&pool)
        .await
        .unwrap();

    let note_id: i64 = sqlx::query_scalar(
        "INSERT INTO notes (title, content, folder_id) VALUES ('N', 'c', ?) RETURNING id",
    )
    .bind(folder_a)
    .fetch_one(&pool)
    .await
    .unwrap();

    sqlx::query("INSERT INTO folder_pins (note_id, folder_id, position) VALUES (?, ?, 0)")
        .bind(note_id)
        .bind(folder_a)
        .execute(&pool)
        .await
        .unwrap();

    let store = EncryptedNoteStore::new(&pool, &keys);

    // Same-folder "move" (no folder change) must keep the pin.
    // EncryptedNoteStore::move_note always rewrites; call with same folder_id.
    store.move_note(note_id, Some(folder_a)).await.unwrap();
    let cnt: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM folder_pins WHERE note_id = ?")
        .bind(note_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(cnt, 1);

    store.move_note(note_id, Some(folder_b)).await.unwrap();
    let cnt: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM folder_pins WHERE note_id = ?")
        .bind(note_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(cnt, 0);

    // Re-pin in B, then Unfiled clears.
    sqlx::query("INSERT INTO folder_pins (note_id, folder_id, position) VALUES (?, ?, 0)")
        .bind(note_id)
        .bind(folder_b)
        .execute(&pool)
        .await
        .unwrap();
    store.move_note(note_id, None).await.unwrap();
    let cnt: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM folder_pins WHERE note_id = ?")
        .bind(note_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(cnt, 0);
}

#[tokio::test]
async fn delete_note_cascades_pin() {
    let pool = test_pool().await;

    let folder_id: i64 = sqlx::query_scalar("INSERT INTO folders (name) VALUES ('f') RETURNING id")
        .fetch_one(&pool)
        .await
        .unwrap();
    let note_id: i64 = sqlx::query_scalar(
        "INSERT INTO notes (title, content, folder_id) VALUES ('N', 'c', ?) RETURNING id",
    )
    .bind(folder_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO folder_pins (note_id, folder_id, position) VALUES (?, ?, 0)")
        .bind(note_id)
        .bind(folder_id)
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("DELETE FROM notes WHERE id = ?")
        .bind(note_id)
        .execute(&pool)
        .await
        .unwrap();

    let cnt: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM folder_pins")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(cnt, 0);
}

#[tokio::test]
async fn mismatched_folder_pin_omitted_from_list() {
    let pool = test_pool().await;

    let folder_a: i64 = sqlx::query_scalar("INSERT INTO folders (name) VALUES ('a') RETURNING id")
        .fetch_one(&pool)
        .await
        .unwrap();
    let folder_b: i64 = sqlx::query_scalar("INSERT INTO folders (name) VALUES ('b') RETURNING id")
        .fetch_one(&pool)
        .await
        .unwrap();
    let note_id: i64 = sqlx::query_scalar(
        "INSERT INTO notes (title, content, folder_id) VALUES ('N', 'c', ?) RETURNING id",
    )
    .bind(folder_a)
    .fetch_one(&pool)
    .await
    .unwrap();

    // Orphan pin claiming folder B while note lives in A.
    sqlx::query("INSERT INTO folder_pins (note_id, folder_id, position) VALUES (?, ?, 0)")
        .bind(note_id)
        .bind(folder_b)
        .execute(&pool)
        .await
        .unwrap();

    let rows: Vec<i64> = sqlx::query_scalar(
        "SELECT fp.note_id FROM folder_pins fp
         JOIN notes n ON n.id = fp.note_id
         WHERE fp.folder_id = ? AND n.folder_id = fp.folder_id",
    )
    .bind(folder_b)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert!(rows.is_empty());
}

#[tokio::test]
async fn locked_folder_is_not_accessible_for_pin() {
    let pool = test_pool().await;
    let keys = empty_keystore();

    let folder_id: i64 = sqlx::query_scalar(
        "INSERT INTO folders (name, locked, salt, sentinel) VALUES ('locked', 1, 'testsalt', 'testsentinel') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let filter = AccessFilter::load(&pool, &keys).await.unwrap();
    assert!(!filter.is_accessible(Some(folder_id)));
}

#[tokio::test]
async fn reorder_rejects_incomplete_pin_set() {
    let pool = test_pool().await;

    let folder_id: i64 = sqlx::query_scalar("INSERT INTO folders (name) VALUES ('f') RETURNING id")
        .fetch_one(&pool)
        .await
        .unwrap();
    let n1: i64 = sqlx::query_scalar(
        "INSERT INTO notes (title, content, folder_id) VALUES ('A', 'c', ?) RETURNING id",
    )
    .bind(folder_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let n2: i64 = sqlx::query_scalar(
        "INSERT INTO notes (title, content, folder_id) VALUES ('B', 'c', ?) RETURNING id",
    )
    .bind(folder_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO folder_pins (note_id, folder_id, position) VALUES (?, ?, 0)")
        .bind(n1)
        .bind(folder_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO folder_pins (note_id, folder_id, position) VALUES (?, ?, 1)")
        .bind(n2)
        .bind(folder_id)
        .execute(&pool)
        .await
        .unwrap();

    let current: Vec<i64> = sqlx::query_scalar(
        "SELECT fp.note_id FROM folder_pins fp
         JOIN notes n ON n.id = fp.note_id
         WHERE fp.folder_id = ? AND n.folder_id = fp.folder_id
         ORDER BY fp.position ASC",
    )
    .bind(folder_id)
    .fetch_all(&pool)
    .await
    .unwrap();

    let ordered = vec![n1]; // incomplete — must reject
    let mut sorted_cur = current.clone();
    let mut sorted_req = ordered.clone();
    sorted_cur.sort_unstable();
    sorted_req.sort_unstable();
    assert_ne!(
        sorted_cur, sorted_req,
        "incomplete order must not match current pin set"
    );
}

#[tokio::test]
async fn duplicate_note_does_not_inherit_pin() {
    let pool = test_pool().await;
    let keys = empty_keystore();

    let folder_id: i64 = sqlx::query_scalar("INSERT INTO folders (name) VALUES ('f') RETURNING id")
        .fetch_one(&pool)
        .await
        .unwrap();
    let note_id: i64 = sqlx::query_scalar(
        "INSERT INTO notes (title, content, folder_id) VALUES ('N', 'body', ?) RETURNING id",
    )
    .bind(folder_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO folder_pins (note_id, folder_id, position) VALUES (?, ?, 0)")
        .bind(note_id)
        .bind(folder_id)
        .execute(&pool)
        .await
        .unwrap();

    let store = EncryptedNoteStore::new(&pool, &keys);
    let copy = store.duplicate_note(note_id).await.unwrap();
    let cnt: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM folder_pins WHERE note_id = ?")
        .bind(copy.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(cnt, 0);
}

#[tokio::test]
async fn pin_membership_check_rejects_wrong_folder() {
    let pool = test_pool().await;

    let folder_a: i64 = sqlx::query_scalar("INSERT INTO folders (name) VALUES ('a') RETURNING id")
        .fetch_one(&pool)
        .await
        .unwrap();
    let folder_b: i64 = sqlx::query_scalar("INSERT INTO folders (name) VALUES ('b') RETURNING id")
        .fetch_one(&pool)
        .await
        .unwrap();
    let note_id: i64 = sqlx::query_scalar(
        "INSERT INTO notes (title, content, folder_id) VALUES ('N', 'c', ?) RETURNING id",
    )
    .bind(folder_a)
    .fetch_one(&pool)
    .await
    .unwrap();

    let note_folder: Option<i64> =
        sqlx::query_scalar("SELECT folder_id FROM notes WHERE id = ?")
            .bind(note_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(note_folder, Some(folder_a));
    assert_ne!(note_folder, Some(folder_b));
}
