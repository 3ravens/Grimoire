//! Chat personas storage + command-level integrity (T-005).

mod common;

use app_lib::{
    create_persona_impl, delete_persona_impl, list_personas_impl, resolve_active_persona_impl,
    update_persona_impl, AppError,
};
use common::test_pool;

#[tokio::test]
async fn migration_creates_chat_personas_table() {
    let pool = test_pool().await;
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'chat_personas'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn list_includes_three_builtin_ids() {
    let pool = test_pool().await;
    let list = list_personas_impl(&pool).await.unwrap();
    let ids: Vec<_> = list.iter().map(|p| p.id.as_str()).collect();
    assert!(ids.contains(&"builtin:blunt-editor"));
    assert!(ids.contains(&"builtin:socratic-tutor"));
    assert!(ids.contains(&"builtin:cite-notes-only"));
    assert!(list.iter().filter(|p| p.builtin).count() >= 3);
}

#[tokio::test]
async fn create_validate_and_resolve_round_trip() {
    let pool = test_pool().await;

    let err = create_persona_impl(&pool, "  ".into(), "body".into(), false)
        .await
        .unwrap_err();
    assert!(matches!(err, AppError::InvalidInput(_)));

    let long_body = "x".repeat(4001);
    let err = create_persona_impl(&pool, "T".into(), long_body, false)
        .await
        .unwrap_err();
    assert!(matches!(err, AppError::InvalidInput(_)));

    let created = create_persona_impl(
        &pool,
        "Tutor".into(),
        "Ask questions.".into(),
        true,
    )
    .await
    .unwrap();
    assert!(created.id.starts_with("user:"));
    assert!(created.also_apply_verbosity);
    assert!(!created.builtin);

    sqlx::query(
        "INSERT INTO settings (key, value) VALUES ('chat_active_persona', ?)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
    )
    .bind(&created.id)
    .execute(&pool)
    .await
    .unwrap();

    let resolved = resolve_active_persona_impl(&pool).await.unwrap().unwrap();
    assert_eq!(resolved.id, created.id);
    assert_eq!(resolved.body, "Ask questions.");
}

#[tokio::test]
async fn delete_active_persona_clears_setting_and_resolve() {
    let pool = test_pool().await;

    let created = create_persona_impl(&pool, "Temp".into(), "Body text here.".into(), false)
        .await
        .unwrap();

    sqlx::query(
        "INSERT INTO settings (key, value) VALUES ('chat_active_persona', ?)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
    )
    .bind(&created.id)
    .execute(&pool)
    .await
    .unwrap();

    delete_persona_impl(&pool, created.id.clone()).await.unwrap();

    let after: String = sqlx::query_scalar(
        "SELECT value FROM settings WHERE key = 'chat_active_persona' LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(after, "");

    assert!(resolve_active_persona_impl(&pool).await.unwrap().is_none());
}

#[tokio::test]
async fn resolve_unknown_id_clears_to_none() {
    let pool = test_pool().await;

    sqlx::query(
        "INSERT INTO settings (key, value) VALUES ('chat_active_persona', 'user:999999')
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
    )
    .execute(&pool)
    .await
    .unwrap();

    assert!(resolve_active_persona_impl(&pool).await.unwrap().is_none());

    let after: String = sqlx::query_scalar(
        "SELECT value FROM settings WHERE key = 'chat_active_persona' LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(after, "");
}

#[tokio::test]
async fn builtins_cannot_be_updated_or_deleted() {
    let pool = test_pool().await;
    let id = "builtin:blunt-editor".to_string();

    let err = update_persona_impl(&pool, id.clone(), "X".into(), "Y".into(), false)
        .await
        .unwrap_err();
    assert!(matches!(err, AppError::InvalidInput(_)));

    let err = delete_persona_impl(&pool, id).await.unwrap_err();
    assert!(matches!(err, AppError::InvalidInput(_)));
}

#[tokio::test]
async fn builtins_are_not_seeded_as_rows() {
    let pool = test_pool().await;
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM chat_personas")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}
