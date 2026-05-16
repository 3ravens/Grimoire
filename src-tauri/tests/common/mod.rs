//! Shared helpers for integration tests (`tests/*.rs`).
#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use app_lib::{KeyStore, SharedKeyStore};
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;

/// Single-connection in-memory SQLite pool with migrations applied.
pub async fn test_pool() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("connect sqlite memory");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("run migrations");

    pool
}

/// Empty session keystore (no vault / folder keys).
pub fn empty_keystore() -> KeyStore {
    KeyStore {
        vault_key: Mutex::new(None),
        folder_keys: Mutex::new(HashMap::new()),
    }
}

pub fn shared_keystore() -> SharedKeyStore {
    Arc::new(empty_keystore())
}
