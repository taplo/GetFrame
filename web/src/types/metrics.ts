export interface MetricsPoint {
  recorded_at: string
  streams_active: number
  frames_ps: number
  errors_decode: number
  errors_storage: number
  errors_kafka: number
  streams_claimed: number
}

export interface MetricsHistoryResponse {
  points: MetricsPoint[]
}
