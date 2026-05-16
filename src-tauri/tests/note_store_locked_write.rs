mod common;

use app_lib::{AppError, EncryptedNoteStore};
use common::{empty_keystore, test_pool};

#[tokio::test]
async fn update_note_rejects_locked_folder_without_key() {
    let pool = test_pool().await;
    let ks = empty_keystore();

    let folder_id: i64 = sqlx::query_scalar(
        "INSERT INTO folders (name, locked) VALUES ('L', 1) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let store = EncryptedNoteStore::new(&pool, &ks);
    let note = store
        .create_note("T", Some(folder_id))
        .await
        .unwrap();

    let err = store
        .update_note(note.id, "T2", "body2")
        .await
        .expect_err("write must fail");
    match err {
        AppError::Auth(msg) => assert_eq!(msg, "folder_locked"),
        other => panic!("unexpected error: {other:?}"),
    }
}

#[tokio::test]
async fn create_get_round_trip_with_folder_encryption_key() {
    let pool = test_pool().await;
    let ks = empty_keystore();

    let folder_id: i64 = sqlx::query_scalar(
        "INSERT INTO folders (name, locked) VALUES ('secret', 1) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    ks.folder_keys
        .lock()
        .unwrap()
        .insert(folder_id, [11u8; 32]);

    let store = EncryptedNoteStore::new(&pool, &ks);
    let note = store
        .create_note("Secret title", Some(folder_id))
        .await
        .unwrap();

    store
        .update_note(note.id, "Secret title", "hidden payload")
        .await
        .unwrap();

    let fetched = store.get_note(note.id).await.unwrap();
    assert_eq!(fetched.title, "Secret title");
    assert_eq!(fetched.content, "hidden payload");
}
