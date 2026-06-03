ALTER TABLE metrics_history
  ADD COLUMN kafka_delta INT NOT NULL DEFAULT 0 AFTER errors_kafka;
