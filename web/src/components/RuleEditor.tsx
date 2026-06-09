import { useState } from "react"
import type { RuleConfig, ComparisonMethod } from "@/types/rule"

const RULE_TYPES = ["interval", "fps", "scene_change", "rate_limited", "static_frame", "composite"] as const

const RULE_LABELS: Record<string, string> = {
  interval: "定时抽帧",
  fps: "固定帧率",
  scene_change: "场景变化",
  rate_limited: "限速",
  static_frame: "静态帧过滤",
  composite: "复合规则",
}

const RULE_PARAM_LABELS: Record<string, string> = {
  interval: "间隔（秒）",
  fps: "FPS",
  scene_change: "阈值 (0.0~1.0)",
  rate_limited: "每分钟上限",
  static_frame: "阈值 (0.0~1.0)",
}

const METHOD_LABELS: Record<ComparisonMethod, string> = {
  pixel_diff: "像素差异 (PixelDiff)",
  perceptual_hash: "感知哈希 (PerceptualHash)",
  ssim: "结构相似度 (SSIM)",
}

function getDefaultParam(type: string): string {
  switch (type) {
    case "interval": return "5"
    case "fps": return "10"
    case "scene_change": return "0.3"
    case "rate_limited": return "30"
    case "static_frame": return "0.005"
    default: return ""
  }
}

function buildRule(type: string, param: string, method: string, force: boolean): RuleConfig {
  const rule: RuleConfig = { type: type as RuleConfig["type"] }
  switch (type) {
    case "interval": rule.interval_seconds = Number(param); break
    case "fps": rule.fps = Number(param); break
    case "scene_change": rule.threshold = Number(param); break
    case "rate_limited": rule.rule = { type: "interval", interval_seconds: 5 }; rule.max_per_minute = Number(param); break
    case "static_frame": rule.threshold = Number(param); rule.method = method as ComparisonMethod; rule.force = force; break
  }
  return rule
}

function ruleSummary(rule: RuleConfig): string {
  switch (rule.type) {
    case "interval": return `每 ${rule.interval_seconds} 秒`
    case "fps": return `${rule.fps} FPS`
    case "scene_change": return `阈值 ${rule.threshold}`
    case "rate_limited": return `限速 ${rule.max_per_minute}/分钟`
    case "static_frame": return `${METHOD_LABELS[rule.method ?? "pixel_diff"]}, ${rule.threshold}${rule.force ? ", 强制" : ""}`
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
  const [editingMethod, setEditingMethod] = useState<string>("pixel_diff")
  const [editingForce, setEditingForce] = useState(false)

  const switchType = (t: string) => {
    setEditingType(t)
    setEditingParam(getDefaultParam(t))
    setEditingMethod("pixel_diff")
    setEditingForce(false)
  }

  const addRule = () => {
    if (!editingType) return
    onChange([...rules, buildRule(editingType, editingParam, editingMethod, editingForce)])
  }

  const removeRule = (index: number) => {
    onChange(rules.filter((_, i) => i !== index))
  }

  const showParamInput = editingType !== "composite" && editingType !== "static_frame"
  const showStaticFrameOptions = editingType === "static_frame"

  return (
    <div className="space-y-3">
      <div>
        <label className="text-sm font-medium block mb-1">规则类型</label>
        <div className="flex gap-2 flex-wrap">
          {RULE_TYPES.map((t) => (
            <button key={t} type="button" onClick={() => switchType(t)}
              className={`px-3 py-1 text-sm border rounded-lg ${editingType === t ? "bg-brand text-white border-brand" : "hover:bg-gray-50"}`}
            >{RULE_LABELS[t]}</button>
          ))}
        </div>
      </div>
      {showStaticFrameOptions && (
        <div className="space-y-2 border rounded-lg p-3 bg-gray-50">
          <div>
            <label className="text-sm font-medium block mb-1">{RULE_PARAM_LABELS.static_frame}</label>
            <input type="number" value={editingParam} onChange={(e) => setEditingParam(e.target.value)}
              className="border rounded-lg px-3 py-1.5 w-full text-sm" step="any" />
          </div>
          <div>
            <label className="text-sm font-medium block mb-1">比较方法</label>
            <select value={editingMethod} onChange={(e) => setEditingMethod(e.target.value)}
              className="border rounded-lg px-3 py-1.5 w-full text-sm">
              {Object.entries(METHOD_LABELS).map(([k, v]) => (
                <option key={k} value={k}>{v}</option>
              ))}
            </select>
          </div>
          <div className="flex items-center gap-2">
            <input type="checkbox" id="static-frame-force" checked={editingForce}
              onChange={(e) => setEditingForce(e.target.checked)}
              className="rounded border-gray-300" />
            <label htmlFor="static-frame-force" className="text-sm text-gray-600">强制抽取（覆盖静态判定）</label>
          </div>
          <button type="button" onClick={addRule} className="px-4 py-1.5 text-sm bg-brand text-white rounded-lg hover:bg-blue-700 whitespace-nowrap w-full">
            添加规则
          </button>
        </div>
      )}
      {showParamInput && (
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
