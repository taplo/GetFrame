import { request } from "./client"
import type { ActivityEvent, ActivityQuery, ActivityListResponse } from "@/types/activity"

function buildQuery(params: ActivityQuery): string {
  const search = new URLSearchParams()
  if (params.event_type) search.set("event_type", params.event_type)
  if (params.resource_type) search.set("resource_type", params.resource_type)
  if (params.actor) search.set("actor", params.actor)
  if (params.search) search.set("search", params.search)
  if (params.since) search.set("since", params.since)
  if (params.until) search.set("until", params.until)
  if (params.page) search.set("page", String(params.page))
  if (params.page_size) search.set("page_size", String(params.page_size))
  const qs = search.toString()
  return qs ? `?${qs}` : ""
}

export const activityApi = {
  list(params: ActivityQuery = {}): Promise<ActivityListResponse> {
    return request<ActivityListResponse>(`/activity${buildQuery(params)}`)
  },

  async exportCsv(params: ActivityQuery = {}): Promise<void> {
    const qs = buildQuery(params)
    const res = await fetch(`/api/v1/activity/export${qs}`)
    if (!res.ok) throw new Error("Export failed")
    const blob = await res.blob()
    const url = URL.createObjectURL(blob)
    const a = document.createElement("a")
    a.href = url
    a.download = `activity-log-${new Date().toISOString().slice(0, 10)}.csv`
    a.click()
    URL.revokeObjectURL(url)
  },
}
