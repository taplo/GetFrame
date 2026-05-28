pub mod registry;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use chrono::Utc;
use registry::{TaskRegistry, TaskId, TaskInfo, TaskStatus, CreateTaskRequest};
use crate::stream::StreamManager;
use crate::types::StreamId;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TaskError {
    #[error("Invalid state transition: {0}")]
    InvalidTransition(String),
    #[error("Task not found")]
    NotFound,
    #[error("Internal error: {0}")]
    Internal(String),
}

pub struct TaskManager {
    registry: TaskRegistry,
    stream_manager: Arc<StreamManager>,
    run_streams: Arc<Mutex<HashMap<TaskId, StreamId>>>,
    pub(crate) db_pool: Option<sqlx::MySqlPool>,
}

impl TaskManager {
    pub fn new(stream_manager: Arc<StreamManager>, db_pool: Option<sqlx::MySqlPool>) -> Self {
        Self {
            registry: TaskRegistry::new(),
            stream_manager,
            run_streams: Arc::new(Mutex::new(HashMap::new())),
            db_pool,
        }
    }

    #[allow(dead_code)]
    pub fn registry(&self) -> &TaskRegistry {
        &self.registry
    }

    pub fn create_task(&self, req: CreateTaskRequest) -> TaskInfo {
        let id = TaskId::new_v4();

        let stream_name = self.stream_manager.registry().get(&req.stream_id)
            .map(|info| info.config.name.clone())
            .unwrap_or_default();

        let info = TaskInfo {
            id,
            name: req.name,
            stream_id: req.stream_id,
            stream_name,
            rules: req.rules,
            status: TaskStatus::Created,
            frames_extracted: 0,
            created_at: Utc::now(),
            started_at: None,
            stopped_at: None,
        };

        self.registry.add(id, info.clone());
        self.persist_task(&info);
        info
    }

    pub fn start_task(&self, id: TaskId) -> Result<TaskInfo, TaskError> {
        let task = self.registry.get(&id).ok_or(TaskError::NotFound)?;
        match task.status {
            TaskStatus::Created | TaskStatus::Error(_) => {}
            ref s => {
                return Err(TaskError::InvalidTransition(format!(
                    "Cannot start task in {:?} state", s
                )));
            }
        }

        let stream_info = self.stream_manager.registry().get(&task.stream_id)
            .ok_or_else(|| TaskError::Internal("Referenced stream not found".into()))?;

        let run_id = StreamId::new_v4();
        self.stream_manager.registry().add(run_id, stream_info.config.clone());
        if !self.stream_manager.start_pipeline(&run_id) {
            self.stream_manager.registry().remove(&run_id);
            return Err(TaskError::Internal("Failed to start pipeline".into()));
        }

        self.run_streams.lock().unwrap().insert(id, run_id);

        let mut task = task;
        task.status = TaskStatus::Running;
        task.started_at = Some(Utc::now());
        self.registry.update_status(&id, TaskStatus::Running);
        self.record_event(id, "Started", None);
        self.persist_task(&task);
        Ok(task)
    }

    pub fn pause_task(&self, id: TaskId) -> Result<TaskInfo, TaskError> {
        let task = self.registry.get(&id).ok_or(TaskError::NotFound)?;
        match task.status {
            TaskStatus::Running => {}
            ref s => {
                return Err(TaskError::InvalidTransition(format!(
                    "Cannot pause task in {:?} state", s
                )));
            }
        }

        if let Some(run_id) = self.run_streams.lock().unwrap().remove(&id) {
            self.stream_manager.stop_pipeline(&run_id);
            self.stream_manager.registry().remove(&run_id);
        }

        let mut task = task;
        task.status = TaskStatus::Paused;
        self.registry.update_status(&id, TaskStatus::Paused);
        self.record_event(id, "Paused", None);
        self.persist_task(&task);
        Ok(task)
    }

    pub fn resume_task(&self, id: TaskId) -> Result<TaskInfo, TaskError> {
        let task = self.registry.get(&id).ok_or(TaskError::NotFound)?;
        match task.status {
            TaskStatus::Paused | TaskStatus::Error(_) => {}
            ref s => {
                return Err(TaskError::InvalidTransition(format!(
                    "Cannot resume task in {:?} state", s
                )));
            }
        }

        let stream_info = self.stream_manager.registry().get(&task.stream_id)
            .ok_or_else(|| TaskError::Internal("Referenced stream not found".into()))?;

        let run_id = StreamId::new_v4();
        self.stream_manager.registry().add(run_id, stream_info.config.clone());
        if !self.stream_manager.start_pipeline(&run_id) {
            self.stream_manager.registry().remove(&run_id);
            return Err(TaskError::Internal("Failed to start pipeline".into()));
        }

        self.run_streams.lock().unwrap().insert(id, run_id);

        let mut task = task;
        task.status = TaskStatus::Running;
        self.registry.update_status(&id, TaskStatus::Running);
        self.record_event(id, "Resumed", None);
        self.persist_task(&task);
        Ok(task)
    }

    pub fn stop_task(&self, id: TaskId) -> Result<TaskInfo, TaskError> {
        let task = self.registry.get(&id).ok_or(TaskError::NotFound)?;
        match task.status {
            TaskStatus::Running | TaskStatus::Paused => {}
            ref s => {
                return Err(TaskError::InvalidTransition(format!(
                    "Cannot stop task in {:?} state", s
                )));
            }
        }

        if let Some(run_id) = self.run_streams.lock().unwrap().remove(&id) {
            self.stream_manager.stop_pipeline(&run_id);
            self.stream_manager.registry().remove(&run_id);
        }

        let mut task = task;
        task.status = TaskStatus::Stopped;
        task.stopped_at = Some(Utc::now());
        self.registry.update_status(&id, TaskStatus::Stopped);
        self.record_event(id, "Stopped", None);
        self.persist_task(&task);
        Ok(task)
    }

    pub fn delete_task(&self, id: TaskId) -> bool {
        if let Some(run_id) = self.run_streams.lock().unwrap().remove(&id) {
            self.stream_manager.stop_pipeline(&run_id);
            self.stream_manager.registry().remove(&run_id);
        }
        self.registry.remove(&id).is_some()
    }

    pub fn get_task(&self, id: TaskId) -> Option<TaskInfo> {
        self.registry.get(&id)
    }

    pub fn list_tasks(&self) -> Vec<TaskInfo> {
        self.registry.list()
    }

    fn record_event(&self, task_id: TaskId, event_type: &str, event_data: Option<serde_json::Value>) {
        let pool = self.db_pool.clone();
        let et = event_type.to_string();
        tokio::spawn(async move {
            if let Some(p) = pool {
                if let Err(e) = crate::db::task_events::insert(&p, &et, &task_id, event_data).await {
                    tracing::warn!(error = %e, task_id = %task_id, event_type = %et, "Failed to record task event");
                }
            }
        });
    }

    fn persist_task(&self, task: &TaskInfo) {
        let pool = self.db_pool.clone();
        let t = task.clone();
        tokio::spawn(async move {
            if let Some(p) = pool {
                let _ = crate::db::tasks::upsert(&p, &t).await;
            }
        });
    }
}
