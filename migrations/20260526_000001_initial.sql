CREATE TABLE IF NOT EXISTS streams (
    id BINARY(16) PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT NOT NULL,
    tags JSON NOT NULL,
    source_url TEXT NOT NULL,
    source_type TEXT NOT NULL,
    stream_type TEXT,
    extract_interval_seconds DOUBLE NOT NULL DEFAULT 5.0,
    jpeg_quality INT NOT NULL DEFAULT 85,
    ffmpeg_threads INT NOT NULL DEFAULT 1,
    rtsp_transport TEXT NOT NULL,
    storage_config JSON,
    kafka_config JSON,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS tasks (
    id BINARY(16) PRIMARY KEY,
    name TEXT NOT NULL,
    stream_id BINARY(16) NOT NULL,
    stream_name VARCHAR(255) NOT NULL,
    status VARCHAR(50) NOT NULL,
    rules JSON NOT NULL,
    frames_extracted BIGINT NOT NULL DEFAULT 0,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    started_at TIMESTAMP NULL,
    stopped_at TIMESTAMP NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE INDEX idx_tasks_stream_id ON tasks(stream_id);
CREATE INDEX idx_tasks_status ON tasks(status);
