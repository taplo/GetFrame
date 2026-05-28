import type { TaskEvent } from "@/api/tasks"

interface EventTimelineProps {
  events: TaskEvent[]
}

const labelMap: Record<string, string> = {
  Started: "启动",
  Paused: "已暂停",
  Resumed: "已恢复",
  Stopped: "已停止",
  Error: "错误",
}

const colorMap: Record<string, string> = {
  Started: "text-green-600 border-green-400",
  Paused: "text-yellow-600 border-yellow-400",
  Resumed: "text-blue-600 border-blue-400",
  Stopped: "text-gray-600 border-gray-400",
  Error: "text-red-600 border-red-400",
}

export function EventTimeline({ events }: EventTimelineProps) {
  if (events.length === 0) {
    return <div className="text-gray-400 text-sm text-center py-8">暂无事件记录</div>
  }

  return (
    <div className="relative">
      <div className="absolute left-4 top-2 bottom-2 w-0.5 bg-gray-200" />

      <div className="space-y-4">
        {events.map((ev, i) => (
          <div key={i} className="flex gap-4 pl-4 relative">
            <div className={`absolute left-2.5 top-1 w-3 h-3 rounded-full border-2 bg-white ${colorMap[ev.event_type] || "border-gray-300"}`} />
            <div className="flex-1 min-w-0">
              <div className="flex items-center gap-2">
                <span className={`text-sm font-medium ${colorMap[ev.event_type]?.split(" ")[0] || "text-gray-700"}`}>
                  {labelMap[ev.event_type] || ev.event_type}
                </span>
                <span className="text-xs text-gray-400">
                  {new Date(ev.recorded_at).toLocaleString("zh-CN")}
                </span>
              </div>
              {ev.event_data && (
                <p className="text-xs text-gray-500 mt-0.5">
                  {String(ev.event_data.message ?? JSON.stringify(ev.event_data))}
                </p>
              )}
            </div>
          </div>
        ))}
      </div>
    </div>
  )
}
