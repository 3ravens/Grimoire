mod common;

use app_lib::EncryptedNoteStore;
use common::{empty_keystore, test_pool};

#[tokio::test]
async fn notes_for_tag_finds_linked_notes() {
    let pool = test_pool().await;
    let ks = empty_keystore();

    let folder: i64 = sqlx::query_scalar("INSERT INTO folders (name) VALUES ('f') RETURNING id")
        .fetch_one(&pool)
        .await
        .unwrap();

    let note_id: i64 = sqlx::query_scalar(
        "INSERT INTO notes (title, content, folder_id) VALUES ('n1', 'body', ?) RETURNING id",
    )
    .bind(folder)
    .fetch_one(&pool)
    .await
    .unwrap();

    sqlx::query("INSERT INTO tags (name) VALUES ('alpha')")
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query(
        "INSERT INTO note_tags (note_id, tag_id) SELECT ?, id FROM tags WHERE name = 'alpha'",
    )
    .bind(note_id)
    .execute(&pool)
    .await
    .unwrap();

    let store = EncryptedNoteStore::new(&pool, &ks);
    let notes = store.notes_for_tag("alpha").await.unwrap();
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].id, note_id);
    assert_eq!(notes[0].title, "n1");
}
