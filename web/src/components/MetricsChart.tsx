import { useMemo } from "react"
import {
  LineChart, Line, BarChart, Bar, XAxis, YAxis, CartesianGrid, Tooltip,
  ResponsiveContainer, Legend,
} from "recharts"
import type { MetricsPoint } from "@/types/metrics"

interface MetricsChartProps {
  points: MetricsPoint[]
}

export function MetricsChart({ points }: MetricsChartProps) {
  const data = useMemo(() => points.map((p) => ({
    time: new Date(p.recorded_at).toLocaleTimeString("zh-CN", { hour: "2-digit", minute: "2-digit" }),
    active: p.streams_active,
    claimed: p.streams_claimed,
    fps: Math.round(p.frames_ps * 10) / 10,
    errDecode: p.errors_decode,
    errStorage: p.errors_storage,
    errKafka: p.errors_kafka,
  })), [points])

  if (data.length === 0) {
    return <div className="text-gray-400 text-sm text-center py-8">暂无指标数据</div>
  }

  return (
    <div className="grid grid-cols-2 gap-6">
      <div className="bg-white border rounded-xl p-5 shadow-sm">
        <h3 className="font-semibold mb-3">活跃流趋势</h3>
        <ResponsiveContainer width="100%" height={200}>
          <LineChart data={data}>
            <CartesianGrid strokeDasharray="3 3" />
            <XAxis dataKey="time" fontSize={12} />
            <YAxis fontSize={12} />
            <Tooltip />
            <Legend />
            <Line type="monotone" dataKey="active" stroke="#2563eb" name="活跃" strokeWidth={2} dot={false} />
            <Line type="monotone" dataKey="claimed" stroke="#7c3aed" name="已认领" strokeWidth={2} dot={false} />
          </LineChart>
        </ResponsiveContainer>
      </div>

      <div className="bg-white border rounded-xl p-5 shadow-sm">
        <h3 className="font-semibold mb-3">抽帧速率</h3>
        <ResponsiveContainer width="100%" height={200}>
          <LineChart data={data}>
            <CartesianGrid strokeDasharray="3 3" />
            <XAxis dataKey="time" fontSize={12} />
            <YAxis fontSize={12} />
            <Tooltip />
            <Legend />
            <Line type="monotone" dataKey="fps" stroke="#059669" name="帧/秒" strokeWidth={2} dot={false} />
          </LineChart>
        </ResponsiveContainer>
      </div>

      <div className="bg-white border rounded-xl p-5 shadow-sm col-span-2">
        <h3 className="font-semibold mb-3">错误率（60s 窗口）</h3>
        <ResponsiveContainer width="100%" height={200}>
          <BarChart data={data}>
            <CartesianGrid strokeDasharray="3 3" />
            <XAxis dataKey="time" fontSize={12} />
            <YAxis fontSize={12} />
            <Tooltip />
            <Legend />
            <Bar dataKey="errDecode" fill="#ef4444" name="解码" stackId="a" />
            <Bar dataKey="errStorage" fill="#f59e0b" name="存储" stackId="a" />
            <Bar dataKey="errKafka" fill="#6366f1" name="Kafka" stackId="a" />
          </BarChart>
        </ResponsiveContainer>
      </div>
    </div>
  )
}
