import { useState, useEffect } from "react"
import { healthApi } from "@/api/health"
import { Heart, HeartOff, AlertTriangle, Clock, Hash } from "lucide-react"

type HealthState = "healthy" | "degraded" | "unhealthy"

interface SystemHealthData {
  status: HealthState
  statusText: string
  uptime: string
  version: string
  activeStreams: number
  ready: boolean
}

function formatUptime(seconds: number): string {
  const d = Math.floor(seconds / 86400)
  const h = Math.floor((seconds % 86400) / 3600)
  const m = Math.floor((seconds % 3600) / 60)
  return d > 0 ? `${d}d ${h}h ${m}m` : h > 0 ? `${h}h ${m}m` : `${m}m`
}

function getHealthData(): Promise<SystemHealthData> {
  return Promise.all([
    healthApi.health().catch(() => null),
    healthApi.ready().catch(() => null),
  ]).then(([health, ready]) => {
    if (!health) {
      return { status: "unhealthy", statusText: "无法连接", uptime: "-", version: "-", activeStreams: 0, ready: false }
    }
    const isReady = ready?.ready ?? true
    const status: HealthState = health.status === "healthy" ? (isReady ? "healthy" : "degraded") : "unhealthy"
    const text = status === "healthy" ? "健康" : status === "degraded" ? "降级" : "故障"
    return {
      status,
      statusText: text,
      uptime: formatUptime((health as any).uptime_seconds || 0),
      version: (health as any).version || "-",
      activeStreams: (health as any).active_streams || 0,
      ready: isReady,
    }
  })
}

const colors: Record<HealthState, { bg: string; dot: string; text: string; icon: typeof Heart }> = {
  healthy: { bg: "bg-green-50 border-green-200", dot: "bg-green-500", text: "text-green-700", icon: Heart },
  degraded: { bg: "bg-yellow-50 border-yellow-200", dot: "bg-yellow-500", text: "text-yellow-700", icon: AlertTriangle },
  unhealthy: { bg: "bg-red-50 border-red-200", dot: "bg-red-500", text: "text-red-700", icon: HeartOff },
}

export function SystemHealth() {
  const [data, setData] = useState<SystemHealthData | null>(null)

  useEffect(() => {
    getHealthData().then(setData)
    const id = setInterval(() => getHealthData().then(setData), 10000)
    return () => clearInterval(id)
  }, [])

  if (!data) {
    return (
      <div className="border rounded-xl p-4 bg-gray-50 animate-pulse">
        <div className="h-5 w-32 bg-gray-200 rounded" />
      </div>
    )
  }

  const c = colors[data.status]
  const Icon = c.icon

  return (
    <div className={`border rounded-xl p-4 ${c.bg}`}>
      <div className="flex items-center gap-3">
        <div className={`w-3 h-3 rounded-full ${c.dot} shrink-0`} />
        <Icon className={`w-4 h-4 ${c.text}`} />
        <span className={`font-semibold text-sm ${c.text}`}>
          系统 {data.statusText}
        </span>
        <div className="flex items-center gap-4 ml-auto text-xs text-gray-500">
          <span className="flex items-center gap-1"><Clock className="w-3 h-3" />{data.uptime}</span>
          <span className="flex items-center gap-1"><Hash className="w-3 h-3" />v{data.version}</span>
          <span className="flex items-center gap-1"><span className={`w-2 h-2 rounded-full ${data.ready ? "bg-green-400" : "bg-yellow-400"}`} />{data.ready ? "就绪" : "未就绪"}</span>
        </div>
      </div>
    </div>
  )
}
