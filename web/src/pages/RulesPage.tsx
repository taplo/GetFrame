import { useState, useEffect, useCallback } from "react"
import { useNavigate } from "react-router-dom"
import { rulesApi } from "@/api/rules"
import { streamsApi } from "@/api/streams"
import type { GlobalRuleItem, RuleConfig, ComparisonMethod } from "@/types/rule"
import type { StreamInfo } from "@/types/stream"

const METHOD_LABELS: Record<ComparisonMethod, string> = {
  pixel_diff: "PixelDiff",
  perceptual_hash: "PerceptualHash",
  ssim: "SSIM",
}

const RULE_LABELS: Record<string, string> = {
  interval: "定时抽帧",
  fps: "固定帧率",
  scene_change: "场景变化",
  rate_limited: "限速",
  static_frame: "静态帧过滤",
  composite: "复合规则",
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

export default function RulesPage() {
  const [items, setItems] = useState<GlobalRuleItem[]>([])
  const [streams, setStreams] = useState<StreamInfo[]>([])
  const [filterStreamId, setFilterStreamId] = useState("")
  const [filterType, setFilterType] = useState("")
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const navigate = useNavigate()

  const load = useCallback(() => {
    setLoading(true)
    setError(null)
    const params: { stream_id?: string; type?: string } = {}
    if (filterStreamId) params.stream_id = filterStreamId
    if (filterType) params.type = filterType
    rulesApi.listGlobal(params)
      .then((res) => setItems(res.rules))
      .catch((e) => setError(e instanceof Error ? e.message : "加载失败"))
      .finally(() => setLoading(false))
  }, [filterStreamId, filterType])

  useEffect(() => { load() }, [load])
  useEffect(() => { streamsApi.list().then((res) => setStreams(res.streams)).catch(() => {}) }, [])

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold">规则管理</h1>
      </div>

      <div className="flex gap-3 items-center">
        <select value={filterStreamId} onChange={(e) => setFilterStreamId(e.target.value)} className="border rounded-lg px-3 py-1.5 text-sm">
          <option value="">全部流</option>
          {streams.map((s) => <option key={s.id} value={s.id}>{s.name}</option>)}
        </select>
        <select value={filterType} onChange={(e) => setFilterType(e.target.value)} className="border rounded-lg px-3 py-1.5 text-sm">
          <option value="">全部类型</option>
          {Object.entries(RULE_LABELS).map(([k, v]) => <option key={k} value={k}>{v}</option>)}
        </select>
        <button onClick={load} className="px-3 py-1.5 text-sm border rounded-lg hover:bg-gray-50">刷新</button>
      </div>

      {error && (
        <div className="bg-red-50 border border-red-200 rounded-lg px-4 py-3 text-sm text-red-700 flex items-center justify-between">
          <span>{error}</span>
          <button onClick={load} className="underline hover:no-underline">重试</button>
        </div>
      )}

      <div className="bg-white border rounded-xl shadow-sm overflow-hidden">
        {loading ? (
          <div className="p-8 text-center text-gray-400">加载中...</div>
        ) : items.length === 0 ? (
          <div className="p-8 text-center text-gray-400">暂无规则</div>
        ) : (
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b bg-gray-50 text-left">
                <th className="px-4 py-3 font-medium text-gray-600">流名称</th>
                <th className="px-4 py-3 font-medium text-gray-600">规则类型</th>
                <th className="px-4 py-3 font-medium text-gray-600">配置</th>
                <th className="px-4 py-3 font-medium text-gray-600">索引</th>
              </tr>
            </thead>
            <tbody>
              {items.map((item) => (
                <tr key={`${item.stream_id}-${item.index}`} className="border-b last:border-0 hover:bg-gray-50">
                  <td className="px-4 py-3">
                    <button onClick={() => navigate(`/streams/${item.stream_id}`)} className="text-brand hover:underline font-medium text-left">{item.stream_name}</button>
                    <div className="text-xs text-gray-400 mt-0.5 truncate max-w-xs">{item.source_url}</div>
                  </td>
                  <td className="px-4 py-3">
                    <span className="px-1.5 py-0.5 rounded text-xs font-medium bg-blue-50 text-blue-700">
                      {RULE_LABELS[item.rule.type] || item.rule.type}
                    </span>
                  </td>
                  <td className="px-4 py-3 text-gray-600">{ruleSummary(item.rule)}</td>
                  <td className="px-4 py-3 text-gray-400 text-xs">{item.index}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>
    </div>
  )
}
