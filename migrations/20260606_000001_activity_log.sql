CREATE TABLE IF NOT EXISTS activity_log (
  id           BIGINT AUTO_INCREMENT PRIMARY KEY,
  event_type   VARCHAR(50)   NOT NULL,
  resource_type VARCHAR(30)  NOT NULL,
  resource_id  VARCHAR(36),
  actor        VARCHAR(64)   NOT NULL DEFAULT 'system',
  description  TEXT          NOT NULL,
  details      JSON,
  recorded_at  TIMESTAMP(6)  NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
  INDEX idx_activity_type (event_type),
  INDEX idx_activity_resource (resource_type, resource_id),
  INDEX idx_activity_recorded (recorded_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
