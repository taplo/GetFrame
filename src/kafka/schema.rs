use apache_avro::types::Value;
use apache_avro::Schema;
use crate::types::FrameMetadata;

pub static SCHEMA_RAW: &str = r#"{
  "type": "record",
  "name": "FrameMetadata",
  "namespace": "com.getframe",
  "fields": [
    {"name": "stream_id",      "type": "string"},
    {"name": "source_type",    "type": "string"},
    {"name": "timestamp",      "type": "string"},
    {"name": "frame_number",   "type": "long"},
    {"name": "rule_trigger",   "type": "string"},
    {"name": "pts",            "type": "long"},
    {"name": "storage_url",    "type": "string"},
    {"name": "storage_bucket", "type": "string"},
    {"name": "storage_key",    "type": "string"},
    {"name": "jpeg_size_bytes","type": "long"},
    {"name": "jpeg_width",     "type": "int"},
    {"name": "jpeg_height",    "type": "int"}
  ]
}"#;

pub static SCHEMA: std::sync::LazyLock<Schema> = std::sync::LazyLock::new(|| {
    let value: serde_json::Value = serde_json::from_str(SCHEMA_RAW)
        .expect("FrameMetadata Avro schema is valid JSON");
    Schema::parse(&value).expect("FrameMetadata Avro schema is valid")
});

pub fn frame_metadata_to_avro_value(meta: &FrameMetadata) -> Value {
    Value::Record(vec![
        ("stream_id".into(),       Value::String(meta.stream_id.clone())),
        ("source_type".into(),     Value::String(meta.source_type.clone())),
        ("timestamp".into(),       Value::String(meta.timestamp.clone())),
        ("frame_number".into(),    Value::Long(meta.frame_number as i64)),
        ("rule_trigger".into(),    Value::String(meta.rule_trigger.clone())),
        ("pts".into(),             Value::Long(meta.pts)),
        ("storage_url".into(),     Value::String(meta.storage_url.clone())),
        ("storage_bucket".into(),  Value::String(meta.storage_bucket.clone())),
        ("storage_key".into(),     Value::String(meta.storage_key.clone())),
        ("jpeg_size_bytes".into(), Value::Long(meta.jpeg_size_bytes as i64)),
        ("jpeg_width".into(),      Value::Int(meta.jpeg_width as i32)),
        ("jpeg_height".into(),     Value::Int(meta.jpeg_height as i32)),
    ])
}
