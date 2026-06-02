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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_raw_valid_json() {
        let value: serde_json::Value = serde_json::from_str(SCHEMA_RAW).unwrap();
        assert_eq!(value["type"], "record");
        assert_eq!(value["name"], "FrameMetadata");
        let fields = value["fields"].as_array().unwrap();
        assert_eq!(fields.len(), 12);
    }

    #[test]
    fn test_schema_parses() {
        let _schema = &*SCHEMA;
    }

    #[test]
    fn test_frame_metadata_to_avro_value() {
        let meta = FrameMetadata {
            stream_id: "test-stream".into(),
            source_type: "rtsp".into(),
            timestamp: "2026-06-01T00:00:00Z".into(),
            frame_number: 42,
            rule_trigger: "interval".into(),
            pts: 1260,
            storage_url: "http://minio:9000/bucket/key.jpg".into(),
            storage_bucket: "test-bucket".into(),
            storage_key: "test-stream/42.jpg".into(),
            jpeg_size_bytes: 50000,
            jpeg_width: 1920,
            jpeg_height: 1080,
        };

        let value = frame_metadata_to_avro_value(&meta);
        match value {
            Value::Record(ref fields) => {
                let map: std::collections::HashMap<&str, &Value> = fields.iter()
                    .map(|(k, v)| (k.as_str(), v))
                    .collect();
                assert_eq!(map.get("stream_id"), Some(&&Value::String("test-stream".into())));
                assert_eq!(map.get("frame_number"), Some(&&Value::Long(42)));
                assert_eq!(map.get("jpeg_width"), Some(&&Value::Int(1920)));
                assert_eq!(map.get("jpeg_height"), Some(&&Value::Int(1080)));
                assert_eq!(map.get("rule_trigger"), Some(&&Value::String("interval".into())));
            }
            _ => panic!("Expected Value::Record"),
        }
    }

    #[test]
    fn test_avro_roundtrip() {
        let meta = FrameMetadata {
            stream_id: "stream-1".into(),
            source_type: "file".into(),
            timestamp: "2026-06-01T12:00:00Z".into(),
            frame_number: 1,
            rule_trigger: "scene_change".into(),
            pts: 30,
            storage_url: "s3://bucket/key.jpg".into(),
            storage_bucket: "bucket".into(),
            storage_key: "stream-1/1.jpg".into(),
            jpeg_size_bytes: 1000,
            jpeg_width: 640,
            jpeg_height: 480,
        };

        let value = frame_metadata_to_avro_value(&meta);
        let mut writer = apache_avro::Writer::new(&SCHEMA, Vec::new());
        writer.append(value).unwrap();
        let encoded = writer.into_inner().unwrap();

        let reader = apache_avro::Reader::new(std::io::Cursor::new(&encoded[..])).unwrap();
        let values: Vec<apache_avro::types::Value> = reader.collect::<Result<Vec<_>, _>>().unwrap();
        assert_eq!(values.len(), 1);

        match &values[0] {
            Value::Record(fields) => {
                let map: std::collections::HashMap<&str, &Value> = fields.iter()
                    .map(|(k, v)| (k.as_str(), v))
                    .collect();
                assert_eq!(map.get("stream_id"), Some(&&Value::String("stream-1".into())));
                assert_eq!(map.get("frame_number"), Some(&&Value::Long(1)));
                assert_eq!(map.get("rule_trigger"), Some(&&Value::String("scene_change".into())));
                assert_eq!(map.get("jpeg_width"), Some(&&Value::Int(640)));
                assert_eq!(map.get("jpeg_height"), Some(&&Value::Int(480)));
            }
            _ => panic!("Expected Value::Record"),
        }
    }
}
