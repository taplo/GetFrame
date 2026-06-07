import { useState, useEffect } from "react"
import { tasksApi } from "@/api/tasks"
import { streamsApi } from "@/api/streams"
import type { StreamInfo } from "@/types/stream"
import type { RuleConfig } from "@/types/rule"
import { RuleEditor } from "./RuleEditor"

export function TaskForm({ onClose, onSave }: { onClose: () => void; onSave: () => void }) {
  const [name, setName] = useState("")
  const [streamId, setStreamId] = useState("")
  const [rules, setRules] = useState<RuleConfig[]>([])
  const [streams, setStreams] = useState<StreamInfo[]>([])
  const [saving, setSaving] = useState(false)

  useEffect(() => {
    streamsApi.list().then((res) => setStreams(res.streams)).catch(() => {})
  }, [])

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    if (!streamId || rules.length === 0) return
    setSaving(true)
    try {
      await tasksApi.create({ name, stream_id: streamId, rules })
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
          <RuleEditor rules={rules} onChange={setRules} />
          <div className="flex justify-end gap-3 pt-2">
            <button type="button" onClick={onClose} className="px-4 py-1.5 text-sm border rounded-lg hover:bg-gray-50">取消</button>
            <button type="submit" disabled={saving || rules.length === 0} className="px-4 py-1.5 text-sm bg-brand text-white rounded-lg hover:bg-blue-700 disabled:opacity-50">
              {saving ? "创建中..." : "创建任务"}
            </button>
          </div>
        </form>
      </div>
    </div>
  )
}
