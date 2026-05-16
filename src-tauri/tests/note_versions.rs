mod common;

use app_lib::EncryptedNoteStore;
use common::{empty_keystore, test_pool};

#[tokio::test]
async fn save_note_with_version_records_prior_snapshot() {
    let pool = test_pool().await;
    let ks = empty_keystore();
    let store = EncryptedNoteStore::new(&pool, &ks);

    let note = store.create_note("Hello", None).await.unwrap();
    store
        .update_note(note.id, "Hello", "first body")
        .await
        .unwrap();

    store
        .save_note_with_version(note.id, "Hello", "second body")
        .await
        .unwrap();

    let versions = store.get_note_versions(note.id).await.unwrap();
    assert_eq!(versions.len(), 1);
    let (_id, _created, _enc, preview_title, preview_body) = &versions[0];
    assert_eq!(preview_title, "Hello");
    assert!(preview_body.contains("first"));

    let current = store.get_note(note.id).await.unwrap();
    assert_eq!(current.content, "second body");
}

#[tokio::test]
async fn get_note_version_content_round_trip() {
    let pool = test_pool().await;
    let ks = empty_keystore();
    let store = EncryptedNoteStore::new(&pool, &ks);

    let note = store.create_note("T", None).await.unwrap();
    store.update_note(note.id, "T", "v1").await.unwrap();
    store
        .save_note_with_version(note.id, "T", "v2")
        .await
        .unwrap();

    let versions = store.get_note_versions(note.id).await.unwrap();
    let vid = versions[0].0;
    let (title, content, _) = store
        .get_note_version_content(note.id, vid)
        .await
        .unwrap();
    assert_eq!(title, "T");
    assert_eq!(content, "v1");
}
