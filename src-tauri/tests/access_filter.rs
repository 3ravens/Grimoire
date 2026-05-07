mod common;

use app_lib::{AccessFilter, EncryptedNoteStore};
use common::{empty_keystore, test_pool};

#[tokio::test]
async fn locked_folder_without_session_key_is_inaccessible() {
    let pool = test_pool().await;
    let ks = empty_keystore();

    let folder_id: i64 = sqlx::query_scalar(
        "INSERT INTO folders (name, locked) VALUES ('secret', 1) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let filter = AccessFilter::load(&pool, &ks).await;
    assert!(!filter.is_accessible(Some(folder_id)));
}

#[tokio::test]
async fn locked_folder_with_session_key_is_accessible() {
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
        .insert(folder_id, [7u8; 32]);

    let filter = AccessFilter::load(&pool, &ks).await;
    assert!(filter.is_accessible(Some(folder_id)));
}

#[tokio::test]
async fn unlocked_folder_always_accessible() {
    let pool = test_pool().await;
    let ks = empty_keystore();

    let folder_id: i64 = sqlx::query_scalar(
        "INSERT INTO folders (name, locked) VALUES ('open', 0) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let filter = AccessFilter::load(&pool, &ks).await;
    assert!(filter.is_accessible(Some(folder_id)));
}

#[tokio::test]
async fn note_without_folder_always_accessible() {
    let pool = test_pool().await;
    let ks = empty_keystore();
    let filter = AccessFilter::load(&pool, &ks).await;
    assert!(filter.is_accessible(None));
}

#[tokio::test]
async fn list_notes_masks_locked_folder_without_key() {
    let pool = test_pool().await;
    let ks = empty_keystore();

    let folder_id: i64 = sqlx::query_scalar(
        "INSERT INTO folders (name, locked) VALUES ('L', 1) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    sqlx::query("INSERT INTO notes (title, content, folder_id) VALUES ('t', 'c', ?)")
        .bind(folder_id)
        .execute(&pool)
        .await
        .unwrap();

    let store = EncryptedNoteStore::new(&pool, &ks);
    let notes = store.list_notes(None, true).await.unwrap();
    assert_eq!(notes.len(), 1);
    assert!(notes[0].locked);
    assert!(notes[0].title.is_empty());
}
