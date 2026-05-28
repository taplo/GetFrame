mod streams;
mod rules;
mod tasks;
mod metrics;

use std::sync::Arc;
use axum::Router;
use sqlx::MySqlPool;
use utoipa::OpenApi;
use crate::stream::StreamManager;
use crate::task::TaskManager;

#[derive(OpenApi)]
#[openapi(
    info(
        title = "GetFrame API",
        description = "High-performance video frame extraction platform API",
        version = "0.1.0",
        license(name = "MIT")
    ),
    paths(
        crate::health::health_handler,
        crate::health::ready_handler,
        crate::api::streams::list_streams,
        crate::api::streams::create_stream,
        crate::api::streams::get_stream,
        crate::api::streams::update_stream,
        crate::api::streams::delete_stream,
        crate::api::streams::test_connection,
        crate::api::streams::get_latest_frame,
        crate::api::rules::list_rules,
        crate::api::rules::create_rule,
        crate::api::rules::get_rule,
        crate::api::rules::update_rule,
        crate::api::rules::delete_rule,
        crate::api::tasks::list_tasks,
        crate::api::tasks::create_task,
        crate::api::tasks::get_task,
        crate::api::tasks::delete_task,
        crate::api::tasks::start_task,
        crate::api::tasks::pause_task,
        crate::api::tasks::resume_task,
        crate::api::tasks::stop_task,
        crate::api::tasks::get_task_events,
        crate::api::metrics::history_handler,
    ),
    components(schemas(
        crate::health::HealthResponse,
        crate::health::ReadyResponse,
        crate::api::streams::StreamResponse,
        crate::api::streams::CreateStreamRequest,
        crate::api::streams::UpdateStreamRequest,
        crate::api::streams::StreamListResponse,
        crate::api::rules::RulesResponse,
        crate::api::rules::CreateRuleRequest,
        crate::api::rules::UpdateRuleRequest,
        crate::api::rules::RuleOperationResponse,
        crate::api::tasks::TaskListResponse,
        crate::task::registry::TaskInfo,
        crate::task::registry::TaskStatus,
        crate::task::registry::CreateTaskRequest,
        crate::config::StreamConfig,
        crate::config::StorageConfig,
        crate::config::KafkaConfig,
        crate::pipeline::rule::RuleConfig,
        crate::pipeline::rule::CompositeOperator,
        crate::api::metrics::MetricsHistoryResponse,
        crate::api::metrics::MetricsPointResponse,
        crate::api::metrics::HistoryQuery,
        crate::api::tasks::TaskEventsResponse,
        crate::api::tasks::TaskEventItem,
    ))
)]
pub struct ApiDoc;

pub fn api_router(manager: StreamManager, task_manager: Arc<TaskManager>, db_pool: Option<MySqlPool>) -> Router {
    let mut router = Router::new()
        .nest("/api/v1/streams", streams::stream_routes(manager.clone()))
        .nest("/api/v1/streams/{id}/rules", rules::rules_routes(manager))
        .nest("/api/v1/tasks", tasks::task_routes(task_manager));

    if let Some(pool) = db_pool {
        router = router.nest("/api/v1/metrics", metrics::metrics_routes(Arc::new(pool)));
    }

    router
}
