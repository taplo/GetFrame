import { useState, useEffect } from "react"
import { useParams, useNavigate } from "react-router-dom"
import { ArrowLeft } from "lucide-react"
import { tasksApi } from "@/api/tasks"
import { streamsApi } from "@/api/streams"
import { FramePreview } from "@/components/FramePreview"
import type { TaskInfo } from "@/types/task"
import type { StreamInfo } from "@/types/stream"

const statusLabel: Record<string, string> = { Created: "已创建", Running: "运行中", Paused: "已暂停", Stopped: "已停止", Error: "异常" }
const statusStyle: Record<string, string> = {
  Running: "text-green-700 bg-green-50 border-green-200",
  Paused: "text-yellow-700 bg-yellow-50 border-yellow-200",
  Stopped: "text-gray-600 bg-gray-50 border-gray-200",
  Error: "text-red-700 bg-red-50 border-red-200",
  Created: "text-blue-700 bg-blue-50 border-blue-200",
}

export function TaskDetail() {
  const { id } = useParams<{ id: string }>()
  const navigate = useNavigate()
  const [task, setTask] = useState<TaskInfo | null>(null)
  const [stream, setStream] = useState<StreamInfo | null>(null)
  const [refreshToken, setRefreshToken] = useState(0)

  useEffect(() => {
    if (!id) return
    tasksApi.get(id).then((t) => {
      setTask(t)
      return streamsApi.get(t.stream_id).then(setStream).catch(() => {})
    }).catch(() => {})
  }, [id])

  const handleAction = async (action: "start" | "pause" | "resume" | "stop" | "delete") => {
    if (!id) return
    if (action === "delete" && !confirm("确定删除此任务？")) return
    const api = tasksApi[action] as (id: string) => Promise<TaskInfo>
    const updated = await api(id)
    setTask(updated)
    setRefreshToken((t) => t + 1)
  }

  if (!task) {
    return (
      <div className="space-y-6">
        <button onClick={() => navigate(-1)} className="flex items-center gap-1 text-sm text-gray-500 hover:text-gray-900">
          <ArrowLeft className="w-4 h-4" />返回
        </button>
        <p className="text-gray-400">加载中...</p>
      </div>
    )
  }

  const actionBtns: { label: string; action: "start" | "pause" | "resume" | "stop" | "delete" }[] = []
  if (task.status === "Created") actionBtns.push({ label: "启动", action: "start" })
  if (task.status === "Running") actionBtns.push({ label: "暂停", action: "pause" })
  if (task.status === "Paused") actionBtns.push({ label: "恢复", action: "resume" })
  if (task.status === "Running" || task.status === "Paused") actionBtns.push({ label: "停止", action: "stop" })
  if (task.status !== "Running" && task.status !== "Paused") actionBtns.push({ label: "删除", action: "delete" })

  return (
    <div className="space-y-6">
      <button onClick={() => navigate(-1)} className="flex items-center gap-1 text-sm text-gray-500 hover:text-gray-900">
        <ArrowLeft className="w-4 h-4" />返回
      </button>

      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold">{task.name}</h1>
        <div className="flex gap-2">
          {actionBtns.map(({ label, action }) => (
            <button key={action} onClick={() => handleAction(action)}
              className="px-3 py-1.5 text-sm border rounded-lg hover:bg-gray-50">
              {label}
            </button>
          ))}
        </div>
      </div>

      <div className="grid grid-cols-3 gap-4">
        <div className="bg-white border rounded-xl p-5 shadow-sm">
          <h3 className="text-xs text-gray-500 uppercase mb-1">状态</h3>
          <span className={`inline-block px-2 py-1 rounded text-sm font-medium border ${statusStyle[task.status] || ""}`}>
            {statusLabel[task.status]}
          </span>
        </div>
        <div className="bg-white border rounded-xl p-5 shadow-sm">
          <h3 className="text-xs text-gray-500 uppercase mb-1">抽帧数</h3>
          <p className="text-2xl font-bold">{task.frames_extracted?.toLocaleString() || "0"}</p>
        </div>
        <div className="bg-white border rounded-xl p-5 shadow-sm">
          <h3 className="text-xs text-gray-500 uppercase mb-1">关联流</h3>
          <p className="text-lg font-medium truncate" title={task.stream_name}>
            {task.stream_name || "-"}
          </p>
        </div>
      </div>

      <div className="grid grid-cols-2 gap-6">
        <div className="bg-white border rounded-xl p-5 shadow-sm">
          <h2 className="font-semibold mb-3">任务信息</h2>
          <dl className="space-y-2 text-sm">
            <div className="flex justify-between"><dt className="text-gray-500">ID</dt><dd className="font-mono text-xs">{task.id}</dd></div>
            <div className="flex justify-between"><dt className="text-gray-500">规则类型</dt><dd>{task.rules?.[0]?.type || "-"}</dd></div>
            <div className="flex justify-between"><dt className="text-gray-500">创建时间</dt><dd>{task.created_at ? new Date(task.created_at).toLocaleString("zh-CN") : "-"}</dd></div>
            {task.started_at && <div className="flex justify-between"><dt className="text-gray-500">开始时间</dt><dd>{new Date(task.started_at).toLocaleString("zh-CN")}</dd></div>}
            {task.stopped_at && <div className="flex justify-between"><dt className="text-gray-500">停止时间</dt><dd>{new Date(task.stopped_at).toLocaleString("zh-CN")}</dd></div>}
          </dl>
        </div>

        <div className="bg-white border rounded-xl p-5 shadow-sm">
          <h2 className="font-semibold mb-3">最新帧</h2>
          <FramePreview
            streamId={task.stream_id}
            latestFrameKey={stream?.latest_frame_key}
            refreshToken={refreshToken}
            className="w-full aspect-video rounded-lg border"
          />
        </div>
      </div>
    </div>
  )
}
