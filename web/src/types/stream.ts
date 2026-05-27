export type StreamStatus = "online" | "offline" | "error" | "connecting"

export interface StreamInfo {
  id: string
  name: string
  source_url: string
  source_type: string
  status: StreamStatus
  tags: Record<string, string>
  description: string
  last_online: string | null
  last_error: string | null
  error_count: number
  uptime_seconds: number
  frames_decoded: number
  frames_extracted: number
  frames_per_hour: number
  reconnect_count: number
  latest_frame_key: string | null
  created_at: string
}

export interface StreamConfig {
  name: string
  description: string
  tags: Record<string, string>
  source_url: string
  source_type: string
  extract_interval_seconds: number
  jpeg_quality: number
  ffmpeg_threads: number
  rtsp_transport: string
}

export interface CreateStreamRequest {
  config: StreamConfig
}

export interface UpdateStreamRequest {
  config: Partial<StreamConfig>
}
