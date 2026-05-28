import { request } from "./client"
import type { TaskInfo, CreateTaskRequest } from "@/types/task"

export interface TaskEvent {
  event_type: string
  event_data: Record<string, unknown> | null
  recorded_at: string
}

export interface TaskEventsResponse {
  events: TaskEvent[]
}

export const tasksApi = {
  list: (params?: { status?: string }) => {
    const qs = params?.status ? `?status=${params.status}` : ""
    return request<{ tasks: TaskInfo[] }>(`/tasks${qs}`)
  },
  get: (id: string) => request<TaskInfo>(`/tasks/${id}`),
  create: (data: CreateTaskRequest) =>
    request<TaskInfo>("/tasks", { method: "POST", body: JSON.stringify(data) }),
  delete: (id: string) =>
    request<void>(`/tasks/${id}`, { method: "DELETE" }),
  start: (id: string) =>
    request<TaskInfo>(`/tasks/${id}/start`, { method: "POST" }),
  pause: (id: string) =>
    request<TaskInfo>(`/tasks/${id}/pause`, { method: "POST" }),
  resume: (id: string) =>
    request<TaskInfo>(`/tasks/${id}/resume`, { method: "POST" }),
  stop: (id: string) =>
    request<TaskInfo>(`/tasks/${id}/stop`, { method: "POST" }),
  events: (id: string) =>
    request<TaskEventsResponse>(`/tasks/${id}/events`),
}
