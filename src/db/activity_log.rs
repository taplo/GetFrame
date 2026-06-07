use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, MySqlPool, QueryBuilder};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct ActivityLogRow {
    pub id: i64,
    pub event_type: String,
    pub resource_type: String,
    pub resource_id: Option<String>,
    pub actor: String,
    pub description: String,
    pub details: Option<serde_json::Value>,
    pub recorded_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct ActivityLogQuery {
    pub event_type: Option<String>,
    pub resource_type: Option<String>,
    pub actor: Option<String>,
    pub search: Option<String>,
    pub since: Option<DateTime<Utc>>,
    pub until: Option<DateTime<Utc>>,
    pub page: i64,
    pub page_size: i64,
}

impl Default for ActivityLogQuery {
    fn default() -> Self {
        Self {
            event_type: None,
            resource_type: None,
            actor: None,
            search: None,
            since: None,
            until: None,
            page: 1,
            page_size: 50,
        }
    }
}

pub async fn insert(pool: &MySqlPool, row: &ActivityLogRow) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO activity_log (event_type, resource_type, resource_id, actor, description, details, recorded_at) VALUES (?, ?, ?, ?, ?, ?, ?)"
    )
    .bind(&row.event_type)
    .bind(&row.resource_type)
    .bind(&row.resource_id)
    .bind(&row.actor)
    .bind(&row.description)
    .bind(&row.details)
    .bind(row.recorded_at)
    .execute(pool)
    .await?;
    Ok(())
}

fn build_filtered_query<'a>(q: &'a ActivityLogQuery, select: &str) -> QueryBuilder<'a, sqlx::MySql> {
    let mut builder = QueryBuilder::new(select);
    let mut sep = " WHERE ";
    if let Some(ref et) = q.event_type {
        builder.push(sep).push("event_type = ").push_bind(et);
        sep = " AND ";
    }
    if let Some(ref rt) = q.resource_type {
        builder.push(sep).push("resource_type = ").push_bind(rt);
        sep = " AND ";
    }
    if let Some(ref a) = q.actor {
        builder.push(sep).push("actor = ").push_bind(a);
        sep = " AND ";
    }
    if let Some(ref s) = q.search {
        builder.push(sep).push("description LIKE ").push_bind(format!("%{}%", s));
        sep = " AND ";
    }
    if let Some(ref since) = q.since {
        builder.push(sep).push("recorded_at >= ").push_bind(since);
        sep = " AND ";
    }
    if let Some(ref until) = q.until {
        builder.push(sep).push("recorded_at <= ").push_bind(until);
    }
    builder
}

pub async fn query(
    pool: &MySqlPool,
    q: &ActivityLogQuery,
) -> Result<(Vec<ActivityLogRow>, i64), sqlx::Error> {
    let page = q.page.max(1);
    let page_size = q.page_size.max(1);
    let offset = (page - 1) * page_size;

    let mut count_builder = build_filtered_query(q, "SELECT COUNT(*) FROM activity_log");
    let total: (i64,) = count_builder.build_query_as().fetch_one(pool).await?;

    let mut data_builder = build_filtered_query(q,
        "SELECT id, event_type, resource_type, resource_id, actor, description, details, recorded_at FROM activity_log"
    );
    data_builder.push(" ORDER BY recorded_at DESC");
    data_builder.push(" LIMIT ").push_bind(q.page_size).push(" OFFSET ").push_bind(offset);
    let rows: Vec<ActivityLogRow> = data_builder.build_query_as().fetch_all(pool).await?;

    Ok((rows, total.0))
}

pub async fn query_export(
    pool: &MySqlPool,
    q: &ActivityLogQuery,
    limit: i64,
) -> Result<Vec<ActivityLogRow>, sqlx::Error> {
    let mut q2 = q.clone();
    q2.page = 1;
    q2.page_size = limit.max(1);
    let (rows, _) = query(pool, &q2).await?;
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_activity_log_row_roundtrip() {
        let row = ActivityLogRow {
            id: 1,
            event_type: "stream.created".into(),
            resource_type: "stream".into(),
            resource_id: Some("uuid".into()),
            actor: "admin".into(),
            description: "创建流 \"Camera-01\"".into(),
            details: None,
            recorded_at: chrono::Utc::now(),
        };
        assert_eq!(row.event_type, "stream.created");
        assert_eq!(row.actor, "admin");
    }

    #[test]
    fn test_activity_log_query_struct() {
        let q = ActivityLogQuery {
            event_type: Some("stream.created".into()),
            resource_type: None,
            actor: None,
            search: None,
            since: None,
            until: None,
            page: 1,
            page_size: 50,
        };
        assert_eq!(q.page, 1);
        assert_eq!(q.page_size, 50);
        assert_eq!(q.event_type.unwrap(), "stream.created");
    }

    #[test]
    fn test_build_filtered_query_no_filters() {
        let q = ActivityLogQuery::default();
        let builder = build_filtered_query(&q, "SELECT * FROM activity_log");
        let sql = builder.sql();
        assert!(!sql.contains("WHERE"));
    }

    #[test]
    fn test_build_filtered_query_with_filters() {
        let q = ActivityLogQuery {
            event_type: Some("stream.created".into()),
            resource_type: Some("stream".into()),
            actor: None,
            search: None,
            since: None,
            until: None,
            page: 1,
            page_size: 50,
        };
        let builder = build_filtered_query(&q, "SELECT * FROM activity_log");
        let sql = builder.sql();
        assert!(sql.contains("WHERE"));
        assert!(sql.contains("event_type ="));
        assert!(sql.contains("resource_type ="));
    }
}
