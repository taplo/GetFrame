import { request } from "./client"
import type { StreamInfo, StreamConfig, CreateStreamRequest } from "@/types/stream"

export interface TestUrlResult {
  reachable: boolean
  latency_ms: number
  detected_type: string | null
  error: string | null
  message: string
}

export interface TestUrlRequest {
  url: string
  source_type?: string
  rtsp_transport?: string
}

export const streamsApi = {
  list: (params?: { status?: string; search?: string }) => {
    const qs = new URLSearchParams()
    if (params?.status) qs.set("status", params.status)
    if (params?.search) qs.set("search", params.search)
    const q = qs.toString()
    return request<{ streams: StreamInfo[] }>(`/streams${q ? `?${q}` : ""}`)
  },
  get: (id: string) => request<StreamInfo>(`/streams/${id}`),
  create: (config: StreamConfig) =>
    request<StreamInfo>("/streams", {
      method: "POST",
      body: JSON.stringify({ config } satisfies CreateStreamRequest),
    }),
  update: (id: string, config: Partial<StreamConfig>) =>
    request<StreamInfo>(`/streams/${id}`, {
      method: "PUT",
      body: JSON.stringify({ config }),
    }),
  delete: (id: string) =>
    request<void>(`/streams/${id}`, { method: "DELETE" }),
  testConnection: (params: TestUrlRequest) =>
    request<TestUrlResult>("/streams/test-url", { method: "POST", body: JSON.stringify(params) }),
}
