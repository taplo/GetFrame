import { request } from "./client"
import type { MetricsHistoryResponse } from "@/types/metrics"

export const metricsApi = {
  history: (minutes = 30) =>
    request<MetricsHistoryResponse>(`/metrics/history?minutes=${minutes}`),
}
