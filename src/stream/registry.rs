use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use crate::config::StreamConfig;
use crate::pipeline::rule::RuleConfig;
use crate::stream::health::StreamHealth;
use crate::types::StreamId;

#[derive(Debug, Clone)]
pub struct StreamInfo {
    pub id: StreamId,
    pub config: StreamConfig,
    pub health: StreamHealth,
    pub rules: Arc<RwLock<Vec<RuleConfig>>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

struct RegistryInner {
    streams: HashMap<StreamId, StreamInfo>,
}

#[derive(Clone)]
pub struct StreamRegistry {
    inner: Arc<RwLock<RegistryInner>>,
}

impl StreamRegistry {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(RegistryInner {
                streams: HashMap::new(),
            })),
        }
    }

    pub fn add(&self, id: StreamId, config: StreamConfig) {
        let mut inner = self.inner.write().unwrap();
        let default_rule = RuleConfig::Interval {
            interval_seconds: config.extract_interval_seconds,
        };
        let info = StreamInfo {
            id,
            config,
            health: StreamHealth::new(),
            rules: Arc::new(RwLock::new(vec![default_rule])),
            created_at: chrono::Utc::now(),
        };
        inner.streams.insert(id, info);
    }

    pub fn remove(&self, id: &StreamId) -> Option<StreamInfo> {
        let mut inner = self.inner.write().unwrap();
        inner.streams.remove(id)
    }

    pub fn get(&self, id: &StreamId) -> Option<StreamInfo> {
        let inner = self.inner.read().unwrap();
        inner.streams.get(id).cloned()
    }

    pub fn list(&self) -> Vec<StreamInfo> {
        let inner = self.inner.read().unwrap();
        inner.streams.values().cloned().collect()
    }

    #[allow(dead_code)]
    pub fn update_health(&self, id: &StreamId, health: StreamHealth) {
        let mut inner = self.inner.write().unwrap();
        if let Some(info) = inner.streams.get_mut(id) {
            info.health = health;
        }
    }

    pub fn update_config(&self, id: &StreamId, config: StreamConfig) -> bool {
        let mut inner = self.inner.write().unwrap();
        if let Some(info) = inner.streams.get_mut(id) {
            info.config = config;
            true
        } else {
            false
        }
    }

    pub fn all_ids(&self) -> Vec<StreamId> {
        let inner = self.inner.read().unwrap();
        inner.streams.keys().copied().collect()
    }

    pub fn exists(&self, id: &StreamId) -> bool {
        let inner = self.inner.read().unwrap();
        inner.streams.contains_key(id)
    }

    pub fn len(&self) -> usize {
        let inner = self.inner.read().unwrap();
        inner.streams.len()
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn get_rules(&self, id: &StreamId) -> Option<Vec<RuleConfig>> {
        let inner = self.inner.read().unwrap();
        inner.streams.get(id).map(|info| {
            info.rules.read().unwrap().clone()
        })
    }

    pub fn update_rules(&self, id: &StreamId, rules: Vec<RuleConfig>) -> bool {
        let inner = self.inner.read().unwrap();
        if let Some(info) = inner.streams.get(id) {
            let mut dest = info.rules.write().unwrap();
            *dest = rules;
            true
        } else {
            false
        }
    }

    pub fn get_rules_shared(&self, id: &StreamId) -> Option<Arc<RwLock<Vec<RuleConfig>>>> {
        let inner = self.inner.read().unwrap();
        inner.streams.get(id).map(|info| info.rules.clone())
    }
}
