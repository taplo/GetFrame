import { useState, useEffect } from "react"
import { tasksApi } from "@/api/tasks"
import { streamsApi } from "@/api/streams"
import type { StreamInfo } from "@/types/stream"
import type { RuleConfig } from "@/types/rule"

const RULE_TYPES = ["interval", "fps", "scene_change", "rate_limited", "composite"] as const

export function TaskForm({ onClose, onSave }: { onClose: () => void; onSave: () => void }) {
  const [name, setName] = useState("")
  const [streamId, setStreamId] = useState("")
  const [ruleType, setRuleType] = useState<string>("interval")
  const [paramValue, setParamValue] = useState("5")
  const [streams, setStreams] = useState<StreamInfo[]>([])
  const [saving, setSaving] = useState(false)

  useEffect(() => {
    streamsApi.list().then((res) => setStreams(res.streams)).catch(() => {})
  }, [])

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    if (!streamId) return
    setSaving(true)
    try {
      const rule: RuleConfig = { type: ruleType as RuleConfig["type"] }
      if (ruleType === "interval") rule.interval_seconds = Number(paramValue)
      else if (ruleType === "fps") rule.fps = Number(paramValue)
      else if (ruleType === "scene_change") rule.threshold = Number(paramValue)
      else if (ruleType === "rate_limited") { rule.rule = { type: "interval", interval_seconds: 5 }; rule.max_per_minute = Number(paramValue) }
      await tasksApi.create({ name, stream_id: streamId, rules: [rule] })
      onSave()
    } finally {
      setSaving(false)
    }
  }

  return (
    <div className="fixed inset-0 bg-black/40 flex items-center justify-center z-50" onClick={onClose}>
      <div className="bg-white rounded-xl p-6 w-full max-w-lg shadow-xl" onClick={(e) => e.stopPropagation()}>
        <h2 className="text-lg font-bold mb-4">新建任务</h2>
        <form onSubmit={handleSubmit} className="space-y-4">
          <div>
            <label className="text-sm font-medium block mb-1">任务名称</label>
            <input required value={name} onChange={(e) => setName(e.target.value)} className="border rounded-lg px-3 py-1.5 w-full text-sm" />
          </div>
          <div>
            <label className="text-sm font-medium block mb-1">关联流</label>
            <select required value={streamId} onChange={(e) => setStreamId(e.target.value)} className="border rounded-lg px-3 py-1.5 w-full text-sm">
              <option value="">选择流...</option>
              {streams.map((s) => <option key={s.id} value={s.id}>{s.name}</option>)}
            </select>
          </div>
          <div>
            <label className="text-sm font-medium block mb-1">规则类型</label>
            <div className="flex gap-2 flex-wrap">
              {RULE_TYPES.map((t) => (
                <button key={t} type="button" onClick={() => setRuleType(t)}
                  className={`px-3 py-1 text-sm border rounded-lg ${ruleType === t ? "bg-brand text-white border-brand" : "hover:bg-gray-50"}`}
                >{t === "interval" ? "间隔" : t === "fps" ? "FPS" : t === "scene_change" ? "场景变化" : t === "rate_limited" ? "限速" : "复合"}</button>
              ))}
            </div>
          </div>
          <div>
            <label className="text-sm font-medium block mb-1">
              {ruleType === "interval" ? "间隔（秒）" : ruleType === "fps" ? "FPS" : ruleType === "scene_change" ? "阈值 (0.0~1.0)" : ruleType === "rate_limited" ? "每分钟上限" : ""}
            </label>
            <input type="number" value={paramValue} onChange={(e) => setParamValue(e.target.value)} className="border rounded-lg px-3 py-1.5 w-full text-sm" step="any" />
          </div>
          <div className="flex justify-end gap-3 pt-2">
            <button type="button" onClick={onClose} className="px-4 py-1.5 text-sm border rounded-lg hover:bg-gray-50">取消</button>
            <button type="submit" disabled={saving} className="px-4 py-1.5 text-sm bg-brand text-white rounded-lg hover:bg-blue-700 disabled:opacity-50">
              {saving ? "创建中..." : "创建任务"}
            </button>
          </div>
        </form>
      </div>
    </div>
  )
}
