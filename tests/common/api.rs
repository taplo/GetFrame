use std::sync::Arc;
use axum::Router;
use sqlx::MySqlPool;
use getframe_worker::stream::StreamManager;
use getframe_worker::task::TaskManager;
use getframe_worker::api::api_router;
use getframe_worker::health::{health_router, HealthState};

pub fn test_app(pool: MySqlPool) -> Router {
    let storage = Arc::new(getframe_worker::storage::StorageClient::noop());
    let kafka = Arc::new(getframe_worker::kafka::KafkaProducer::noop());
    let sm = StreamManager::new(storage, kafka).with_db(pool.clone());
    let tm = Arc::new(TaskManager::new(Arc::new(sm.clone()), Some(pool.clone())));
    let health_state = HealthState::new(Some(Arc::new(sm.registry().clone())));
    health_router(health_state)
        .merge(api_router(sm, tm, Some(pool)))
}
