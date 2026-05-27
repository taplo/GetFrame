use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct SchemaRegistryClient {
    client: reqwest::Client,
    base_url: String,
    subject: String,
}

#[derive(Serialize)]
struct RegisterRequest {
    schema: String,
    schema_type: String,
}

#[derive(Deserialize)]
struct RegisterResponse {
    id: u32,
}

#[derive(Deserialize)]
struct IdResponse {
    id: u32,
}

impl SchemaRegistryClient {
    pub fn new(base_url: &str, subject: &str) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .expect("reqwest Client::builder() always succeeds"),
            base_url: base_url.trim_end_matches('/').to_string(),
            subject: subject.to_string(),
        }
    }

    pub async fn register_schema(&self, schema_json: &str) -> Result<u32> {
        let url = format!("{}/subjects/{}/versions", self.base_url, self.subject);
        let body = RegisterRequest {
            schema: schema_json.to_string(),
            schema_type: "AVRO".into(),
        };

        let resp = self.client
            .post(&url)
            .json(&body)
            .send()
            .await
            .context("Failed to send register-schema request to Schema Registry")?;

        if resp.status().is_success() {
            let reg: RegisterResponse = resp
                .json()
                .await
                .context("Failed to parse Schema Registry register response")?;
            Ok(reg.id)
        } else if resp.status().as_u16() == 409 {
            self.get_schema_id(schema_json).await
        } else {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            anyhow::bail!(
                "Schema Registry register failed (HTTP {}): {}",
                status, text
            );
        }
    }

    async fn get_schema_id(&self, schema_json: &str) -> Result<u32> {
        let url = format!("{}/subjects/{}/versions/latest", self.base_url, self.subject);
        let resp = self.client
            .get(&url)
            .send()
            .await
            .context("Failed to query Schema Registry for latest schema")?;

        if resp.status().is_success() {
            let id_resp: IdResponse = resp
                .json()
                .await
                .context("Failed to parse Schema Registry id response")?;
            return Ok(id_resp.id);
        }

        let url = format!("{}/subjects/{}", self.base_url, self.subject);
        let body = RegisterRequest {
            schema: schema_json.to_string(),
            schema_type: "AVRO".into(),
        };
        let resp = self.client
            .post(&url)
            .json(&body)
            .send()
            .await
            .context("Failed to look up existing schema in Schema Registry")?;

        if resp.status().is_success() {
            let id_resp: IdResponse = resp
                .json()
                .await
                .context("Failed to parse Schema Registry lookup response")?;
            Ok(id_resp.id)
        } else {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            anyhow::bail!(
                "Schema Registry lookup failed (HTTP {}): {}",
                status, text
            );
        }
    }
}
