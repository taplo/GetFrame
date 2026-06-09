export type CompositeOperator = "any" | "all"
export type ComparisonMethod = "pixel_diff" | "perceptual_hash" | "ssim"

export interface RuleConfig {
  type: "interval" | "fps" | "rate_limited" | "scene_change" | "static_frame" | "composite"
  interval_seconds?: number
  fps?: number
  max_per_minute?: number
  threshold?: number
  rule?: RuleConfig
  operator?: CompositeOperator
  rules?: RuleConfig[]
  method?: ComparisonMethod
  force?: boolean
}

export interface GlobalRuleItem {
  stream_id: string
  stream_name: string
  source_url: string
  index: number
  rule: RuleConfig
}
