import { useState } from "react"
import type { RuleConfig } from "@/types/rule"

const RULE_TYPES = ["interval", "fps", "scene_change", "rate_limited", "composite"] as const

const RULE_LABELS: Record<string, string> = {
  interval: "定时抽帧",
  fps: "固定帧率",
  scene_change: "场景变化",
  rate_limited: "限速",
  composite: "复合规则",
}

const RULE_PARAM_LABELS: Record<string, string> = {
  interval: "间隔（秒）",
  fps: "FPS",
  scene_change: "阈值 (0.0~1.0)",
  rate_limited: "每分钟上限",
}

function getDefaultParam(type: string): string {
  switch (type) {
    case "interval": return "5"
    case "fps": return "10"
    case "scene_change": return "0.3"
    case "rate_limited": return "30"
    default: return ""
  }
}

function buildRule(type: string, param: string): RuleConfig {
  const rule: RuleConfig = { type: type as RuleConfig["type"] }
  switch (type) {
    case "interval": rule.interval_seconds = Number(param); break
    case "fps": rule.fps = Number(param); break
    case "scene_change": rule.threshold = Number(param); break
    case "rate_limited": rule.rule = { type: "interval", interval_seconds: 5 }; rule.max_per_minute = Number(param); break
  }
  return rule
}

function ruleSummary(rule: RuleConfig): string {
  switch (rule.type) {
    case "interval": return `每 ${rule.interval_seconds} 秒`
    case "fps": return `${rule.fps} FPS`
    case "scene_change": return `阈值 ${rule.threshold}`
    case "rate_limited": return `限速 ${rule.max_per_minute}/分钟`
    case "composite": return `复合 (${rule.operator})`
  }
}

export function RuleEditor({
  rules,
  onChange,
}: {
  rules: RuleConfig[]
  onChange: (rules: RuleConfig[]) => void
}) {
  const [editingType, setEditingType] = useState<string>("interval")
  const [editingParam, setEditingParam] = useState("5")

  const addRule = () => {
    if (!editingType) return
    onChange([...rules, buildRule(editingType, editingParam)])
  }

  const removeRule = (index: number) => {
    onChange(rules.filter((_, i) => i !== index))
  }

  return (
    <div className="space-y-3">
      <div>
        <label className="text-sm font-medium block mb-1">规则类型</label>
        <div className="flex gap-2 flex-wrap">
          {RULE_TYPES.map((t) => (
            <button key={t} type="button" onClick={() => { setEditingType(t); setEditingParam(getDefaultParam(t)) }}
              className={`px-3 py-1 text-sm border rounded-lg ${editingType === t ? "bg-brand text-white border-brand" : "hover:bg-gray-50"}`}
            >{RULE_LABELS[t]}</button>
          ))}
        </div>
      </div>
      {editingType !== "composite" && (
        <div>
          <label className="text-sm font-medium block mb-1">{RULE_PARAM_LABELS[editingType]}</label>
          <div className="flex gap-2">
            <input type="number" value={editingParam} onChange={(e) => setEditingParam(e.target.value)}
              className="border rounded-lg px-3 py-1.5 w-full text-sm" step="any" />
            <button type="button" onClick={addRule} className="px-4 py-1.5 text-sm bg-brand text-white rounded-lg hover:bg-blue-700 whitespace-nowrap">
              添加规则
            </button>
          </div>
        </div>
      )}
      {rules.length > 0 && (
        <div>
          <label className="text-sm font-medium block mb-1">已添加规则 ({rules.length})</label>
          <ul className="space-y-1">
            {rules.map((rule, i) => (
              <li key={i} className="flex items-center justify-between bg-gray-50 rounded-lg px-3 py-2 text-sm">
                <span><span className="font-medium text-gray-500 mr-2">#{i + 1}</span>{RULE_LABELS[rule.type]} — {ruleSummary(rule)}</span>
                <button type="button" onClick={() => removeRule(i)} className="text-error hover:text-red-800 text-xs font-medium">删除</button>
              </li>
            ))}
          </ul>
        </div>
      )}
    </div>
  )
}
