//! Installation wizard integration tests.

mod common;

use std::sync::{Arc, RwLock};

use app_lib::AppConfig;
use app_lib::connect_dir;
use app_lib::{wizard_finish_impl, wizard_status_impl, WizardFinishResult};
use common::{shared_keystore, test_pool};

#[tokio::test]
async fn wizard_status_empty_vault_shows_wizard() {
    let pool = test_pool().await;
    let s = wizard_status_impl(&pool).await.unwrap();
    assert!(s.show_wizard);
}

#[tokio::test]
async fn wizard_finish_then_idempotent() {
    let pool = test_pool().await;
    let ks = shared_keystore();
    let cfg = Arc::new(RwLock::new(AppConfig::load(&pool).await.unwrap()));

    let dir = tempfile::tempdir().unwrap();
    let conn = connect_dir(dir.path()).await.unwrap();

    let r1 = wizard_finish_impl(
        &pool,
        &ks,
        &cfg,
        &conn,
        "empty".into(),
        false,
        false,
        None,
        None,
    )
    .await
    .unwrap();
    assert!(!r1.already_completed);

    let folders_after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM folders")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(folders_after, 0);

    let r2 = wizard_finish_impl(
        &pool,
        &ks,
        &cfg,
        &conn,
        "para".into(),
        false,
        false,
        None,
        None,
    )
    .await
    .unwrap();
    assert!(r2.already_completed);

    let folders_still: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM folders")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(folders_still, 0);
}

#[tokio::test]
async fn wizard_finish_para_creates_folders() {
    let pool = test_pool().await;
    let ks = shared_keystore();
    let cfg = Arc::new(RwLock::new(AppConfig::load(&pool).await.unwrap()));
    let dir = tempfile::tempdir().unwrap();
    let conn = connect_dir(dir.path()).await.unwrap();

    let r = wizard_finish_impl(
        &pool,
        &ks,
        &cfg,
        &conn,
        "para".into(),
        false,
        false,
        None,
        None,
    )
    .await
    .unwrap();
    let WizardFinishResult {
        ok,
        already_completed,
        ..
    } = r;
    assert!(ok);
    assert!(!already_completed);

    let fc: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM folders")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(fc, 4);

    let nc: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM notes")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(nc, 4);
}

#[tokio::test]
async fn starter_apply_transaction_rolls_back_on_invalid_pack() {
    let pool = test_pool().await;
    let ks = shared_keystore();
    let cfg = Arc::new(RwLock::new(AppConfig::load(&pool).await.unwrap()));
    let dir = tempfile::tempdir().unwrap();
    let conn = connect_dir(dir.path()).await.unwrap();

    let err = wizard_finish_impl(
        &pool,
        &ks,
        &cfg,
        &conn,
        "not_a_real_pack".into(),
        false,
        false,
        None,
        None,
    )
    .await
    .unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("Unknown starter pack") || msg.contains("InvalidInput"));

    let fc: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM folders")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(fc, 0);

    let done: String =
        sqlx::query_scalar("SELECT value FROM settings WHERE key = 'wizard_v1_completed'")
            .fetch_optional(&pool)
            .await
            .unwrap()
            .unwrap_or_default();
    assert_ne!(done, "true");
}
