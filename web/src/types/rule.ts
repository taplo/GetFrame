export type CompositeOperator = "any" | "all"

export interface RuleConfig {
  type: "interval" | "fps" | "rate_limited" | "scene_change" | "composite"
  interval_seconds?: number
  fps?: number
  max_per_minute?: number
  threshold?: number
  rule?: RuleConfig
  operator?: CompositeOperator
  rules?: RuleConfig[]
}
