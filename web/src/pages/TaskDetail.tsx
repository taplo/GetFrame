import { useState, useEffect } from "react"
import { useParams, useNavigate, Link } from "react-router-dom"
import { ArrowLeft } from "lucide-react"
import { tasksApi } from "@/api/tasks"
import { streamsApi } from "@/api/streams"
import { FramePreview } from "@/components/FramePreview"
import { EventTimeline } from "@/components/EventTimeline"
import type { TaskInfo } from "@/types/task"
import type { StreamInfo } from "@/types/stream"
import type { TaskEvent } from "@/api/tasks"
import type { RuleConfig, ComparisonMethod } from "@/types/rule"

const statusLabel: Record<string, string> = { Created: "已创建", Running: "运行中", Paused: "已暂停", Stopped: "已停止", Error: "异常" }
const statusStyle: Record<string, string> = {
  Running: "text-green-700 bg-green-50 border-green-200",
  Paused: "text-yellow-700 bg-yellow-50 border-yellow-200",
  Stopped: "text-gray-600 bg-gray-50 border-gray-200",
  Error: "text-red-700 bg-red-50 border-red-200",
  Created: "text-blue-700 bg-blue-50 border-blue-200",
}

const METHOD_LABELS: Record<ComparisonMethod, string> = {
  pixel_diff: "PixelDiff", perceptual_hash: "PerceptualHash", ssim: "SSIM",
}

const RULE_LABELS: Record<string, string> = {
  interval: "定时抽帧", fps: "固定帧率", scene_change: "场景变化", rate_limited: "限速", static_frame: "静态帧过滤", composite: "复合规则",
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

type LoadState = "loading" | "loaded" | "error" | "not_found"

export function TaskDetail() {
  const { id } = useParams<{ id: string }>()
  const navigate = useNavigate()
  const [task, setTask] = useState<TaskInfo | null>(null)
  const [stream, setStream] = useState<StreamInfo | null>(null)
  const [events, setEvents] = useState<TaskEvent[]>([])
  const [state, setState] = useState<LoadState>("loading")
  const [refreshToken, setRefreshToken] = useState(0)

  const load = () => {
    if (!id) return
    setState("loading")
    tasksApi.get(id).then((t) => {
      setTask(t)
      streamsApi.get(t.stream_id).then(setStream).catch(() => {})
      return t
    }).then(() => {
      tasksApi.events(id!).then((res) => setEvents(res.events)).catch(() => {})
      setState("loaded")
    }).catch((e) => {
      if (e?.status === 404) setState("not_found")
      else setState("error")
    })
  }

  useEffect(() => { load() }, [id, refreshToken])

  const handleAction = async (action: "start" | "pause" | "resume" | "stop" | "delete") => {
    if (!id) return
    if (action === "delete" && !confirm("确定删除此任务？")) return
    const api = tasksApi[action] as (id: string) => Promise<TaskInfo>
    const updated = await api(id)
    setTask(updated)
    setRefreshToken((t) => t + 1)
  }

  if (state === "loading") return (
    <div className="space-y-6">
      <button onClick={() => navigate(-1)} className="flex items-center gap-1 text-sm text-gray-500 hover:text-gray-900">
        <ArrowLeft className="w-4 h-4" />返回
      </button>
      <div className="bg-white border rounded-xl p-8 shadow-sm text-center text-gray-400">加载中...</div>
    </div>
  )

  if (state === "error") return (
    <div className="space-y-6">
      <button onClick={() => navigate(-1)} className="flex items-center gap-1 text-sm text-gray-500 hover:text-gray-900">
        <ArrowLeft className="w-4 h-4" />返回
      </button>
      <div className="bg-red-50 border border-red-200 rounded-xl p-8 text-center">
        <p className="text-red-700 mb-3">加载失败</p>
        <button onClick={() => setRefreshToken((t) => t + 1)} className="px-3 py-1.5 text-sm border border-red-300 rounded-lg hover:bg-red-100">重试</button>
      </div>
    </div>
  )

  if (state === "not_found" || !task) return (
    <div className="space-y-6">
      <button onClick={() => navigate(-1)} className="flex items-center gap-1 text-sm text-gray-500 hover:text-gray-900">
        <ArrowLeft className="w-4 h-4" />返回
      </button>
      <div className="bg-white border rounded-xl p-8 shadow-sm text-center text-gray-400">任务不存在</div>
    </div>
  )

  const actionBtns: { label: string; action: "start" | "pause" | "resume" | "stop" | "delete" }[] = []
  if (task.status === "Created") actionBtns.push({ label: "启动", action: "start" })
  if (task.status === "Running") actionBtns.push({ label: "暂停", action: "pause" })
  if (task.status === "Paused") actionBtns.push({ label: "恢复", action: "resume" })
  if (task.status === "Running" || task.status === "Paused") actionBtns.push({ label: "停止", action: "stop" })
  if (task.status !== "Running" && task.status !== "Paused") actionBtns.push({ label: "删除", action: "delete" })

  const taskDurationMs = task.started_at
    ? (task.stopped_at ? new Date(task.stopped_at).getTime() - new Date(task.started_at).getTime() : Date.now() - new Date(task.started_at).getTime())
    : 0

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
          <Link to={`/streams/${task.stream_id}`} className="text-lg font-medium text-brand hover:underline truncate block" title={task.stream_name}>
            {task.stream_name || "-"}
          </Link>
        </div>
      </div>

      <div className="grid grid-cols-2 gap-6">
        <div className="bg-white border rounded-xl p-5 shadow-sm">
          <h2 className="font-semibold mb-3">任务信息</h2>
          <dl className="space-y-2 text-sm">
            <div className="flex justify-between"><dt className="text-gray-500">ID</dt><dd className="font-mono text-xs">{task.id}</dd></div>
            <div className="flex justify-between items-start">
              <dt className="text-gray-500 pt-0.5">规则</dt>
              <dd className="text-right">
                {task.rules && task.rules.length > 0 ? (
                  <ul className="space-y-1">
                    {task.rules.map((r, i) => (
                      <li key={i} className="text-xs">
                        <span className="text-gray-400 mr-1">#{i + 1}</span>
                        <span className="font-medium">{RULE_LABELS[r.type] || r.type}</span>
                        <span className="text-gray-500 ml-1">— {ruleSummary(r)}</span>
                      </li>
                    ))}
                  </ul>
                ) : <span className="text-gray-400">-</span>}
              </dd>
            </div>
            <div className="flex justify-between"><dt className="text-gray-500">创建时间</dt><dd>{task.created_at ? new Date(task.created_at).toLocaleString("zh-CN") : "-"}</dd></div>
            {task.started_at && <div className="flex justify-between"><dt className="text-gray-500">开始时间</dt><dd>{new Date(task.started_at).toLocaleString("zh-CN")}</dd></div>}
            {task.stopped_at && <div className="flex justify-between"><dt className="text-gray-500">停止时间</dt><dd>{new Date(task.stopped_at).toLocaleString("zh-CN")}</dd></div>}
            <div className="flex justify-between items-center">
              <dt className="text-gray-500">帧率稳定性</dt>
              <dd>
                {task.frames_extracted && task.frames_extracted > 0 && taskDurationMs > 0 ? (
                  (() => {
                    const intervalSeconds = task.rules?.[0]?.type === "interval" ? (task.rules[0] as any).interval_seconds || 1 : 1
                    const expected = Math.round((taskDurationMs / 1000) * (task.rules?.[0]?.type === "interval" ? 1 / intervalSeconds : 1))
                    const actual = task.frames_extracted
                    const ratio = expected > 0 ? Math.min(actual / expected, 2.0) : 1
                    const color = ratio >= 0.95 ? "text-green-700 bg-green-50" : ratio >= 0.8 ? "text-yellow-700 bg-yellow-50" : "text-red-700 bg-red-50"
                    const label = ratio >= 0.95 ? "稳定" : ratio >= 0.8 ? "轻微漂移" : "不稳定"
                    return <span className={`px-2 py-0.5 rounded text-xs font-medium ${color}`}>{label} ({Math.round(ratio * 100)}%)</span>
                  })()
                ) : (
                  <span className="text-gray-400">-</span>
                )}
              </dd>
            </div>
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

      {(task.frames_extracted ?? 0) > 0 && events.length > 0 && (
        <div className="bg-white border rounded-xl p-5 shadow-sm">
          <h2 className="font-semibold mb-3">抽取吞吐量</h2>
          {(() => {
            const buckets = new Map<string, number>()
            events.forEach((ev) => {
              if (ev.event_type === "Started" || ev.event_type === "Resumed") {
                const t = new Date(ev.recorded_at)
                const key = `${t.getFullYear()}-${String(t.getMonth() + 1).padStart(2, "0")}-${String(t.getDate()).padStart(2, "0")} ${String(t.getHours()).padStart(2, "0")}:${String(Math.floor(t.getMinutes() / 5) * 5).padStart(2, "0")}`
                buckets.set(key, (buckets.get(key) || 0) + 1)
              }
            })
            if (buckets.size === 0) return <p className="text-gray-400 text-sm">暂无事件分布数据</p>
            const max = Math.max(...buckets.values(), 1)
            return (
              <>
                <div className="flex items-end gap-1 h-20">
                  {Array.from(buckets.entries()).map(([time, count]) => (
                    <div key={time} className="flex-1 flex flex-col items-center gap-1">
                      <div className="w-full bg-purple-500 rounded-t" style={{ height: `${(count / max) * 100}%`, minHeight: "4px" }} title={`${time}: ${count} 事件`} />
                      <span className="text-[10px] text-gray-400 truncate w-full text-center">{time.split(" ")[1]}</span>
                    </div>
                  ))}
                </div>
                <p className="text-xs text-gray-400 mt-2">每 5 分钟时间槽的事件分布</p>
              </>
            )
          })()}
        </div>
      )}

      <div className="bg-white border rounded-xl p-5 shadow-sm">
        <h2 className="font-semibold mb-3">事件时间线</h2>
        <EventTimeline events={events} />
      </div>
    </div>
  )
}
