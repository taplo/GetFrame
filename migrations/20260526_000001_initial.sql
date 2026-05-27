CREATE TABLE IF NOT EXISTS streams (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL DEFAULT '',
    description TEXT NOT NULL DEFAULT '',
    tags JSONB NOT NULL DEFAULT '{}',
    source_url TEXT NOT NULL,
    source_type TEXT NOT NULL,
    stream_type TEXT,
    extract_interval_seconds DOUBLE PRECISION NOT NULL DEFAULT 5.0,
    jpeg_quality INTEGER NOT NULL DEFAULT 85,
    ffmpeg_threads INTEGER NOT NULL DEFAULT 1,
    rtsp_transport TEXT NOT NULL DEFAULT 'tcp',
    storage_config JSONB,
    kafka_config JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS tasks (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL,
    stream_id UUID NOT NULL,
    stream_name TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT 'Created',
    rules JSONB NOT NULL DEFAULT '[]',
    frames_extracted BIGINT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    started_at TIMESTAMPTZ,
    stopped_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_tasks_stream_id ON tasks(stream_id);
CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks(status);
