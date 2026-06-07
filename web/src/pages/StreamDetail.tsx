import { useState, useEffect, useCallback } from "react"
import { useParams, useNavigate } from "react-router-dom"
import { streamsApi } from "@/api/streams"
import { StreamForm } from "@/components/StreamForm"
import { FramePreview } from "@/components/FramePreview"
import type { StreamInfo } from "@/types/stream"

const STATUS_LABELS: Record<string, string> = { online: "在线", offline: "离线", error: "错误", connecting: "连接中" }
const STATUS_STYLES: Record<string, string> = {
  online: "bg-green-50 text-green-700",
  offline: "bg-gray-50 text-gray-600",
  error: "bg-red-50 text-red-700",
  connecting: "bg-yellow-50 text-yellow-700",
}

export default function StreamDetail() {
  const { id } = useParams<{ id: string }>()
  const navigate = useNavigate()
  const [stream, setStream] = useState<StreamInfo | null>(null)
  const [showEdit, setShowEdit] = useState(false)
  const [loading, setLoading] = useState(true)

  const load = useCallback(() => {
    if (!id) return
    setLoading(true)
    streamsApi.get(id).then(setStream).catch(() => navigate("/streams")).finally(() => setLoading(false))
  }, [id, navigate])

  useEffect(() => { load() }, [load])

  const handleDelete = async () => {
    if (!id || !confirm("确定删除该流？")) return
    await streamsApi.delete(id)
    navigate("/streams")
  }

  if (loading) return <div className="p-8 text-center text-gray-400">加载中...</div>
  if (!stream) return <div className="p-8 text-center text-gray-400">流不存在</div>

  return (
    <div className="space-y-6">
      <button onClick={() => navigate("/streams")} className="text-sm text-brand hover:underline">&larr; 返回流列表</button>

      <div className="bg-white border rounded-xl p-5 shadow-sm">
        <div className="flex justify-between items-start mb-4">
          <div>
            <h1 className="text-xl font-bold">{stream.name}</h1>
            <div className="text-xs text-gray-400 mt-1 font-mono">{stream.id}</div>
          </div>
          <span className={`px-2 py-1 rounded text-xs font-medium ${STATUS_STYLES[stream.status] || ""}`}>
            {STATUS_LABELS[stream.status] || stream.status}
          </span>
        </div>
        <div className="grid grid-cols-2 gap-4 text-sm">
          <div>
            <span className="text-gray-500">源 URL</span>
            <div className="font-mono text-xs mt-0.5 truncate">{stream.source_url}</div>
          </div>
          <div>
            <span className="text-gray-500">类型</span>
            <div>{(stream.source_type || "").toUpperCase()}</div>
          </div>
          <div>
            <span className="text-gray-500">解码帧数</span>
            <div>{stream.frames_decoded}</div>
          </div>
          <div>
            <span className="text-gray-500">抽帧数</span>
            <div>{stream.frames_extracted}</div>
          </div>
          <div>
            <span className="text-gray-500">当前 FPS</span>
            <div>{stream.frames_per_hour ? (stream.frames_per_hour / 3600).toFixed(2) : "—"}</div>
          </div>
          <div>
            <span className="text-gray-500">在线时长</span>
            <div>{stream.uptime_seconds ? `${Math.floor(stream.uptime_seconds / 60)} 分钟` : "—"}</div>
          </div>
        </div>
        <div className="flex gap-2 mt-4 pt-4 border-t">
          <button onClick={() => setShowEdit(true)} className="px-3 py-1.5 text-sm border rounded-lg hover:bg-gray-50">编辑</button>
          <button onClick={handleDelete} className="px-3 py-1.5 text-sm border rounded-lg text-error hover:bg-red-50">删除</button>
        </div>
      </div>

      <div className="bg-white border rounded-xl p-5 shadow-sm">
        <h2 className="text-base font-semibold mb-3">最新帧</h2>
        <FramePreview streamId={id!} />
      </div>

      {showEdit && <StreamForm stream={stream} onClose={() => setShowEdit(false)} onSave={() => { setShowEdit(false); load() }} />}
    </div>
  )
}
