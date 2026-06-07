import { request } from "./client"
import type { GlobalRuleItem } from "@/types/rule"

export const rulesApi = {
  listGlobal: (params?: { stream_id?: string; type?: string }) => {
    const qs = new URLSearchParams()
    if (params?.stream_id) qs.set("stream_id", params.stream_id)
    if (params?.type) qs.set("type", params.type)
    const query = qs.toString()
    return request<{ rules: GlobalRuleItem[] }>(`/rules${query ? `?${query}` : ""}`)
  },
}
