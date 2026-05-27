import type { RuleConfig } from "./rule"

export type TaskStatus = "Created" | "Running" | "Paused" | "Stopped" | "Error"

export interface TaskInfo {
  id: string
  name: string
  stream_id: string
  stream_name: string
  status: TaskStatus
  rules: RuleConfig[]
  frames_extracted: number
  created_at: string
  started_at?: string
  stopped_at?: string
}

export interface CreateTaskRequest {
  name: string
  stream_id: string
  rules: RuleConfig[]
}
