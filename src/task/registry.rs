use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use crate::pipeline::rule::RuleConfig;
use crate::types::StreamId;

pub type TaskId = uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub enum TaskStatus {
    Created,
    Running,
    Paused,
    Stopped,
    Error(String),
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct TaskInfo {
    #[schema(value_type = String)]
    pub id: TaskId,
    pub name: String,
    #[schema(value_type = String)]
    pub stream_id: StreamId,
    pub stream_name: String,
    pub rules: Vec<RuleConfig>,
    pub status: TaskStatus,
    pub frames_extracted: u64,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub stopped_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateTaskRequest {
    pub name: String,
    #[schema(value_type = String)]
    pub stream_id: StreamId,
    pub rules: Vec<RuleConfig>,
}

struct RegistryInner {
    tasks: HashMap<TaskId, TaskInfo>,
}

#[derive(Clone)]
pub struct TaskRegistry {
    inner: Arc<RwLock<RegistryInner>>,
}

impl TaskRegistry {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(RegistryInner {
                tasks: HashMap::new(),
            })),
        }
    }

    pub fn add(&self, id: TaskId, info: TaskInfo) {
        let mut inner = self.inner.write().unwrap();
        inner.tasks.insert(id, info);
    }

    pub fn remove(&self, id: &TaskId) -> Option<TaskInfo> {
        let mut inner = self.inner.write().unwrap();
        inner.tasks.remove(id)
    }

    pub fn get(&self, id: &TaskId) -> Option<TaskInfo> {
        let inner = self.inner.read().unwrap();
        inner.tasks.get(id).cloned()
    }

    pub fn list(&self) -> Vec<TaskInfo> {
        let inner = self.inner.read().unwrap();
        inner.tasks.values().cloned().collect()
    }

    pub fn update_status(&self, id: &TaskId, status: TaskStatus) -> bool {
        let mut inner = self.inner.write().unwrap();
        if let Some(info) = inner.tasks.get_mut(id) {
            info.status = status;
            true
        } else {
            false
        }
    }

    #[allow(dead_code)]
    pub fn update_frames(&self, id: &TaskId, frames_extracted: u64) -> bool {
        let mut inner = self.inner.write().unwrap();
        if let Some(info) = inner.tasks.get_mut(id) {
            info.frames_extracted = frames_extracted;
            true
        } else {
            false
        }
    }

    #[allow(dead_code)]
    pub fn exists(&self, id: &TaskId) -> bool {
        let inner = self.inner.read().unwrap();
        inner.tasks.contains_key(id)
    }

    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        let inner = self.inner.read().unwrap();
        inner.tasks.len()
    }

    #[allow(dead_code)]
    pub fn load_all(&self, tasks: Vec<TaskInfo>) {
        let mut inner = self.inner.write().unwrap();
        for t in tasks {
            inner.tasks.insert(t.id, t);
        }
    }
}
