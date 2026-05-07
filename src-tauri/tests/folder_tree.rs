mod common;

use app_lib::folder_tree;
use common::test_pool;

#[tokio::test]
async fn folder_subtree_includes_self_and_descendants() {
    let pool = test_pool().await;

    let root: i64 = sqlx::query_scalar("INSERT INTO folders (name) VALUES ('root') RETURNING id")
        .fetch_one(&pool)
        .await
        .unwrap();

    let child: i64 = sqlx::query_scalar(
        "INSERT INTO folders (name, parent_id) VALUES ('child', ?) RETURNING id",
    )
    .bind(root)
    .fetch_one(&pool)
    .await
    .unwrap();

    let grand: i64 = sqlx::query_scalar(
        "INSERT INTO folders (name, parent_id) VALUES ('grand', ?) RETURNING id",
    )
    .bind(child)
    .fetch_one(&pool)
    .await
    .unwrap();

    let mut ids = folder_tree::folder_subtree_ids(&pool, root).await.unwrap();
    ids.sort_unstable();
    let mut expected = vec![root, child, grand];
    expected.sort_unstable();
    assert_eq!(ids, expected);
}

#[tokio::test]
async fn folder_subtree_single_node_when_no_children() {
    let pool = test_pool().await;

    let leaf: i64 = sqlx::query_scalar("INSERT INTO folders (name) VALUES ('leaf') RETURNING id")
        .fetch_one(&pool)
        .await
        .unwrap();

    let ids = folder_tree::folder_subtree_ids(&pool, leaf).await.unwrap();
    assert_eq!(ids, vec![leaf]);
}
