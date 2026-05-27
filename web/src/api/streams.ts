import { request } from "./client"
import type { StreamInfo, StreamConfig, CreateStreamRequest } from "@/types/stream"

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
  testConnection: (url: string) =>
    request<{ reachable: boolean }>("/streams/test", { method: "POST", body: JSON.stringify({ url }) }),
}
