CREATE TABLE IF NOT EXISTS metrics_history (
  id              BIGINT AUTO_INCREMENT PRIMARY KEY,
  recorded_at     TIMESTAMP(6) NOT NULL,
  streams_active  INT NOT NULL,
  frames_delta    INT NOT NULL,
  errors_decode   INT NOT NULL,
  errors_storage  INT NOT NULL,
  errors_kafka    INT NOT NULL,
  streams_claimed INT NOT NULL,
  INDEX idx_metrics_recorded (recorded_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS task_events (
  id           BIGINT AUTO_INCREMENT PRIMARY KEY,
  task_id      CHAR(36) NOT NULL,
  event_type   VARCHAR(30) NOT NULL,
  event_data   JSON,
  recorded_at  TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
  INDEX idx_task_events_task (task_id),
  INDEX idx_task_events_recorded (recorded_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
