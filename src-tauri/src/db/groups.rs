//! Sibling-row groups for cross-network publishing.
//!
//! A `post_groups` row is the parent of N `post_history` rows that share
//! the same brief but target different networks. The schema lives in
//! migration 018; the cascade-on-delete semantics are enforced here in
//! Rust because SQLite refuses `ALTER TABLE ... ADD COLUMN ...
//! REFERENCES ...` (see migration 013's hotfix history).
//!
//! Public surface is intentionally small: create the parent + children
//! atomically inside a transaction, fetch the parent with its members,
//! and delete the parent while keeping the children alive as standalone
//! drafts the user can still publish or delete one by one.
//!
//! ## Why `dead_code` is allowed module-wide
//!
//! This module is the foundation of a 4-PR feature stack. The CRUD
//! surface is verified by the tests at the bottom of the file but no
//! Tauri command consumes it yet — the consumer arrives in the next PR
//! (`generate_and_save_group`). Without the allow, clippy `-D warnings`
//! would block CI on a perfectly correct intermediate landing.

#![allow(dead_code)]

use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use super::history::{row_to_post_record, PostRecord};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostGroup {
    pub id: i64,
    pub brief: String,
    pub created_at: String,
    /// Sibling drafts ordered by their `post_history.id` so the network
    /// the user started with always renders first in the UI.
    pub members: Vec<PostRecord>,
}

/// Insert a `post_groups` row and return its new id. The caller is
/// responsible for inserting child `post_history` rows that reference
/// this id within the same transaction; see `create_with_drafts` for
/// the typical multi-network composer flow.
pub async fn create(pool: &SqlitePool, brief: &str) -> Result<i64, String> {
    let now = Utc::now().to_rfc3339();
    sqlx::query("INSERT INTO post_groups (brief, created_at) VALUES (?, ?)")
        .bind(brief)
        .bind(&now)
        .execute(pool)
        .await
        .map(|r| r.last_insert_rowid())
        .map_err(|e| e.to_string())
}

/// Atomically create a group plus N child drafts in one transaction so
/// we never end up with a parent group that has zero children (the
/// composer flow rolls back as a unit on partial failure).
///
/// Each child is described by `(network, caption, hashtags, account_id)`
/// — the same shape as `db::history::insert_draft` but with the new
/// `group_id` column wired up so the dashboard / calendar group view
/// can find them by parent id.
pub async fn create_with_drafts(
    pool: &SqlitePool,
    brief: &str,
    children: &[GroupChildInput],
) -> Result<GroupCreateResult, String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    let now = Utc::now().to_rfc3339();

    let group_row = sqlx::query("INSERT INTO post_groups (brief, created_at) VALUES (?, ?)")
        .bind(brief)
        .bind(&now)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    let group_id = group_row.last_insert_rowid();

    let mut child_ids = Vec::with_capacity(children.len());
    for child in children {
        let hashtags_json = serde_json::to_string(&child.hashtags).map_err(|e| e.to_string())?;
        let images_json = match child.image_path.as_deref() {
            Some(path) if !path.is_empty() => {
                serde_json::to_string(&vec![path.to_string()]).map_err(|e| e.to_string())?
            }
            _ => "[]".to_string(),
        };
        let row = sqlx::query(
            "INSERT INTO post_history \
                (network, caption, hashtags, status, created_at,
                 image_path, images, account_id, group_id) \
             VALUES (?, ?, ?, 'draft', ?, ?, ?, ?, ?)",
        )
        .bind(&child.network)
        .bind(&child.caption)
        .bind(&hashtags_json)
        .bind(&now)
        .bind(child.image_path.as_deref())
        .bind(&images_json)
        .bind(child.account_id)
        .bind(group_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
        child_ids.push(row.last_insert_rowid());
    }

    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(GroupCreateResult {
        group_id,
        child_ids,
    })
}

/// Fetch a group with its sibling drafts. Returns `None` if the group
/// id doesn't exist (legacy mono-network rows have `group_id = NULL`
/// and never collide with a parent id, so a missing parent is a
/// genuine 404, not a retrocompat case).
pub async fn get_with_members(
    pool: &SqlitePool,
    group_id: i64,
) -> Result<Option<PostGroup>, String> {
    let parent = sqlx::query_as::<_, (i64, String, String)>(
        "SELECT id, brief, created_at FROM post_groups WHERE id = ?",
    )
    .bind(group_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;

    let Some((id, brief, created_at)) = parent else {
        return Ok(None);
    };

    let rows = sqlx::query(
        "SELECT id, network, caption, hashtags, status, created_at, published_at,
                scheduled_at, image_path, images, ig_media_id, account_id, published_url
         FROM post_history
         WHERE group_id = ?
         ORDER BY id ASC",
    )
    .bind(group_id)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let members = rows
        .iter()
        .map(row_to_post_record)
        .collect::<Result<Vec<_>, _>>()?;

    Ok(Some(PostGroup {
        id,
        brief,
        created_at,
        members,
    }))
}

/// Delete the group row while keeping its children alive — the SQLite
/// schema has no FK so we manually NULL out `post_history.group_id`
/// before dropping the parent. Wrapped in a transaction so a crash
/// between the UPDATE and the DELETE can't leave dangling references.
pub async fn delete_keeping_children(pool: &SqlitePool, group_id: i64) -> Result<(), String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    sqlx::query("UPDATE post_history SET group_id = NULL WHERE group_id = ?")
        .bind(group_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM post_groups WHERE id = ?")
        .bind(group_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())
}

/// Single child draft to insert when a group is created. `image_path`
/// is optional because the renderer fills it in later (after Playwright
/// runs); the row is created with an empty `images` array and the
/// images column is patched in by `db::history::update_images` when
/// the renders complete.
#[derive(Debug, Clone)]
pub struct GroupChildInput {
    pub network: String,
    pub caption: String,
    pub hashtags: Vec<String>,
    pub image_path: Option<String>,
    pub account_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GroupCreateResult {
    pub group_id: i64,
    pub child_ids: Vec<i64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    /// Fresh in-memory pool with all migrations applied. Mirrors the
    /// helper used by `db::history::tests` and `db::migration_tests`.
    async fn fresh_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("connect in-memory sqlite");
        sqlx::migrate!("src/db/migrations")
            .run(&pool)
            .await
            .expect("migrations apply cleanly");
        pool
    }

    fn child(network: &str, caption: &str) -> GroupChildInput {
        GroupChildInput {
            network: network.to_string(),
            caption: caption.to_string(),
            hashtags: vec!["test".to_string()],
            image_path: None,
            account_id: None,
        }
    }

    #[tokio::test]
    async fn create_with_drafts_inserts_parent_and_children_atomically() {
        let pool = fresh_pool().await;
        let result = create_with_drafts(
            &pool,
            "Test brief — multi network",
            &[
                child("instagram", "IG caption"),
                child("linkedin", "LI caption"),
            ],
        )
        .await
        .expect("create_with_drafts must succeed");

        assert_eq!(result.child_ids.len(), 2);

        let parent_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM post_groups WHERE id = ?")
            .bind(result.group_id)
            .fetch_one(&pool)
            .await
            .expect("count parent");
        assert_eq!(parent_count, 1);

        let child_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM post_history WHERE group_id = ?")
                .bind(result.group_id)
                .fetch_one(&pool)
                .await
                .expect("count children");
        assert_eq!(child_count, 2);
    }

    #[tokio::test]
    async fn get_with_members_returns_group_and_orders_by_id() {
        let pool = fresh_pool().await;
        let result = create_with_drafts(
            &pool,
            "ordered brief",
            &[child("instagram", "first"), child("linkedin", "second")],
        )
        .await
        .expect("create");

        let group = get_with_members(&pool, result.group_id)
            .await
            .expect("fetch")
            .expect("group exists");

        assert_eq!(group.brief, "ordered brief");
        assert_eq!(group.members.len(), 2);
        // Children must come back in insertion order — the network the
        // user started the composer with always renders first in the UI.
        assert_eq!(group.members[0].caption, "first");
        assert_eq!(group.members[1].caption, "second");
    }

    #[tokio::test]
    async fn get_with_members_returns_none_for_missing_id() {
        let pool = fresh_pool().await;
        let group = get_with_members(&pool, 9999).await.expect("fetch");
        assert!(group.is_none());
    }

    #[tokio::test]
    async fn delete_keeping_children_drops_parent_but_orphans_children_safely() {
        // Soft-cascade contract: dropping the parent group must NULL the
        // children's `group_id` and leave them publishable as standalone
        // drafts. The brief lives only on the parent and is acceptable
        // to lose — the captions are what the user actually publishes.
        let pool = fresh_pool().await;
        let result = create_with_drafts(
            &pool,
            "to be deleted",
            &[child("instagram", "keep me"), child("linkedin", "me too")],
        )
        .await
        .expect("create");

        delete_keeping_children(&pool, result.group_id)
            .await
            .expect("delete");

        // Parent gone.
        let parent_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM post_groups WHERE id = ?")
            .bind(result.group_id)
            .fetch_one(&pool)
            .await
            .expect("count parent");
        assert_eq!(parent_count, 0);

        // Children alive with NULL group_id.
        for child_id in &result.child_ids {
            let group_id: Option<i64> =
                sqlx::query_scalar("SELECT group_id FROM post_history WHERE id = ?")
                    .bind(child_id)
                    .fetch_one(&pool)
                    .await
                    .expect("fetch child");
            assert!(
                group_id.is_none(),
                "child {child_id} must have NULL group_id after cascade-soft delete"
            );
            let still_there: i64 =
                sqlx::query_scalar("SELECT COUNT(*) FROM post_history WHERE id = ?")
                    .bind(child_id)
                    .fetch_one(&pool)
                    .await
                    .expect("count child");
            assert_eq!(
                still_there, 1,
                "child {child_id} must survive parent deletion"
            );
        }
    }

    #[tokio::test]
    async fn group_id_column_is_nullable_for_legacy_rows() {
        // Retrocompat guard: the existing single-network composer flow
        // calls `db::history::insert_draft` which leaves group_id unset.
        // Migration 018 must keep that column nullable so legacy rows
        // (and the legacy code path) continue to work without a backfill.
        let pool = fresh_pool().await;
        let id = sqlx::query(
            "INSERT INTO post_history \
                (network, caption, hashtags, status, created_at, images) \
             VALUES ('instagram', 'legacy', '[]', 'draft', '2026-05-10T00:00:00Z', '[]') \
             RETURNING id",
        )
        .fetch_one(&pool)
        .await
        .expect("insert legacy row");
        let id: i64 = sqlx::Row::try_get(&id, "id").expect("read id");

        let group_id: Option<i64> =
            sqlx::query_scalar("SELECT group_id FROM post_history WHERE id = ?")
                .bind(id)
                .fetch_one(&pool)
                .await
                .expect("read group_id");
        assert!(
            group_id.is_none(),
            "legacy single-network row must have NULL group_id"
        );
    }
}
