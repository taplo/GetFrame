import { useState, useEffect } from "react"
import { useNavigate } from "react-router-dom"
import { tasksApi } from "@/api/tasks"
import { streamsApi } from "@/api/streams"
import { RuleEditor } from "@/components/RuleEditor"
import type { StreamInfo } from "@/types/stream"
import type { RuleConfig } from "@/types/rule"

export default function TaskCreatePage() {
  const navigate = useNavigate()
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
      const task = await tasksApi.create({ name, stream_id: streamId, rules })
      navigate(`/tasks/${task.id}`)
    } catch {
      setSaving(false)
    }
  }

  return (
    <div className="max-w-2xl mx-auto">
      <button onClick={() => navigate("/tasks")} className="text-sm text-brand hover:underline mb-4 inline-block">&larr; 返回任务列表</button>
      <h1 className="text-xl font-bold mb-6">新建任务</h1>

      <form onSubmit={handleSubmit} className="space-y-6">
        <div className="bg-white border rounded-xl p-5 shadow-sm space-y-4">
          <div>
            <label className="text-sm font-medium block mb-1">任务名称</label>
            <input required value={name} onChange={(e) => setName(e.target.value)}
              className="border rounded-lg px-3 py-1.5 w-full text-sm" placeholder="输入任务名称" />
          </div>
          <div>
            <label className="text-sm font-medium block mb-1">关联流</label>
            <select required value={streamId} onChange={(e) => setStreamId(e.target.value)}
              className="border rounded-lg px-3 py-1.5 w-full text-sm">
              <option value="">选择流...</option>
              {streams.map((s) => <option key={s.id} value={s.id}>{s.name}</option>)}
            </select>
          </div>
        </div>

        <div className="bg-white border rounded-xl p-5 shadow-sm">
          <h2 className="text-base font-semibold mb-3">抽帧规则</h2>
          <RuleEditor rules={rules} onChange={setRules} />
        </div>

        <div className="flex justify-end gap-3">
          <button type="button" onClick={() => navigate("/tasks")}
            className="px-4 py-1.5 text-sm border rounded-lg hover:bg-gray-50">取消</button>
          <button type="submit" disabled={saving || rules.length === 0}
            className="px-4 py-1.5 text-sm bg-brand text-white rounded-lg hover:bg-blue-700 disabled:opacity-50">
            {saving ? "创建中..." : "创建任务"}
          </button>
        </div>
      </form>
    </div>
  )
}
