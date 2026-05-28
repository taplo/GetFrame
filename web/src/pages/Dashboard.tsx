import { useState, useEffect, useCallback } from "react"
import { Activity, Video, Image, AlertTriangle } from "lucide-react"
import { StatCard } from "@/components/StatCard"
import { FramePreview } from "@/components/FramePreview"
import { streamsApi } from "@/api/streams"
import { tasksApi } from "@/api/tasks"
import { metricsApi } from "@/api/metrics"
import type { StreamInfo } from "@/types/stream"
import type { TaskInfo } from "@/types/task"
import type { MetricsPoint } from "@/types/metrics"
import { MetricsChart } from "@/components/MetricsChart"

export function Dashboard() {
  const [streams, setStreams] = useState<StreamInfo[]>([])
  const [tasks, setTasks] = useState<TaskInfo[]>([])
  const [refreshing, setRefreshing] = useState(false)
  const [metrics, setMetrics] = useState<MetricsPoint[]>([])
  const [refreshToken, setRefreshToken] = useState(0)

  const load = useCallback(() => {
    setRefreshing(true)
    setRefreshToken((t) => t + 1)
    Promise.all([
      streamsApi.list().then((res) => setStreams(res.streams)).catch(() => {}),
      tasksApi.list().then((res) => setTasks(res.tasks)).catch(() => {}),
      metricsApi.history(30).then((res) => setMetrics(res.points)).catch(() => {}),
    ]).finally(() => setRefreshing(false))
  }, [])

  useEffect(() => {
    load()
    const id = setInterval(load, 10000)
    return () => clearInterval(id)
  }, [load])

  const online = streams.filter((s) => s.status === "online").length
  const offline = streams.filter((s) => s.status !== "online").length
  const running = tasks.filter((t) => t.status === "Running").length
  const totalFrames = tasks.reduce((sum, t) => sum + (t.frames_extracted || 0), 0)

  return (
    <div className="space-y-6">
      <h1 className="text-2xl font-bold">控制面板</h1>

      <div className="flex items-center justify-between">
        <div className="grid grid-cols-4 gap-4 flex-1">
          <StatCard title="在线流" value={online} icon={Video} color="success" />
          <StatCard title="活跃任务" value={running} icon={Activity} color="brand" />
          <StatCard title="抽帧总数" value={totalFrames.toLocaleString()} icon={Image} />
          <StatCard title="离线流" value={offline} icon={AlertTriangle} color={offline > 0 ? "error" : "default"} />
        </div>
        <button onClick={load} disabled={refreshing}
          className="ml-4 px-3 py-1.5 text-sm border rounded-lg hover:bg-gray-50 disabled:opacity-50 self-start">
          {refreshing ? "刷新中..." : "刷新"}
        </button>
      </div>

      <MetricsChart points={metrics} />

      <div className="grid grid-cols-2 gap-6">
        <div className="bg-white border rounded-xl p-5 shadow-sm">
          <h2 className="font-semibold mb-3">最近流状态</h2>
          {streams.slice(0, 5).map((s) => (
            <div key={s.id} className="flex items-center gap-3 py-2 border-b last:border-0 text-sm">
              <FramePreview streamId={s.id} latestFrameKey={s.latest_frame_key} refreshToken={refreshToken} className="w-12 h-8 shrink-0" />
              <span className={`w-2 h-2 rounded-full shrink-0 ${s.status === "online" ? "bg-success" : s.status.startsWith("error") ? "bg-error" : "bg-gray-400"}`} />
              <span className="flex-1 truncate">{s.name}</span>
              <span className="text-gray-500">{s.frames_per_hour ? `${s.frames_per_hour} 帧/时` : s.status === "online" ? "在线" : "离线"}</span>
            </div>
          ))}
        </div>

        <div className="bg-white border rounded-xl p-5 shadow-sm">
          <h2 className="font-semibold mb-3">最近任务</h2>
          {tasks.slice(0, 5).map((t) => (
            <div key={t.id} className="flex items-center gap-3 py-2 border-b last:border-0 text-sm">
              <span className={`px-1.5 py-0.5 rounded text-xs font-medium ${
                t.status === "Running" ? "bg-green-100 text-green-700" :
                t.status === "Paused" ? "bg-yellow-100 text-yellow-700" :
                "bg-gray-100 text-gray-600"
              }`}>{t.status === "Running" ? "运行中" : t.status === "Paused" ? "已暂停" : "已停止"}</span>
              <span className="flex-1 truncate">{t.name}</span>
            </div>
          ))}
        </div>
      </div>
    </div>
  )
}
