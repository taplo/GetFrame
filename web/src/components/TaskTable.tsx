import { useState } from "react"
import { Link } from "react-router-dom"
import { tasksApi } from "@/api/tasks"
import type { TaskInfo } from "@/types/task"

const statusLabel: Record<string, string> = { Created: "已创建", Running: "运行中", Paused: "已暂停", Stopped: "已停止", Error: "异常" }
const statusStyle: Record<string, string> = {
  Running: "bg-green-100 text-green-700",
  Paused: "bg-yellow-100 text-yellow-700",
  Stopped: "bg-gray-100 text-gray-600",
  Error: "bg-red-100 text-red-700",
  Created: "bg-blue-100 text-blue-700",
}

interface TaskTableProps {
  tasks: TaskInfo[]
  onRefresh: () => void
}

export function TaskTable({ tasks, onRefresh }: TaskTableProps) {
  const [selected, setSelected] = useState<Set<string>>(new Set())
  const [search, setSearch] = useState("")

  const filtered = search
    ? tasks.filter((t) => t.name.toLowerCase().includes(search.toLowerCase()) || t.stream_name.toLowerCase().includes(search.toLowerCase()))
    : tasks

  const toggle = (id: string) => {
    const next = new Set(selected)
    if (next.has(id)) next.delete(id); else next.add(id)
    setSelected(next)
  }

  const toggleAll = () => {
    if (selected.size === tasks.length) setSelected(new Set())
    else setSelected(new Set(tasks.map((t) => t.id)))
  }

  const handleAction = async (id: string, action: "start" | "pause" | "resume" | "stop" | "delete") => {
    if (action === "delete" && !confirm("确定删除此任务？")) return
    const api = tasksApi[action] as (id: string) => Promise<TaskInfo>
    await api(id)
    onRefresh()
  }

  const handleBatchAction = async (action: "start" | "pause" | "resume" | "stop" | "delete") => {
    if (action === "delete" && !confirm(`确定删除选中的 ${selected.size} 个任务？`)) return
    const api = tasksApi[action] as (id: string) => Promise<TaskInfo>
    for (const id of selected) await api(id)
    setSelected(new Set())
    onRefresh()
  }

  const batchActions: { label: string; action: "start" | "pause" | "resume" | "stop" | "delete" }[] = [
    { label: "启动", action: "start" },
    { label: "暂停", action: "pause" },
    { label: "恢复", action: "resume" },
    { label: "停止", action: "stop" },
    { label: "删除", action: "delete" },
  ]

  return (
    <div className="bg-white border rounded-xl shadow-sm overflow-hidden">
      <div className="p-4 border-b flex items-center justify-between gap-4">
        <input placeholder="搜索任务..." value={search} onChange={(e) => setSearch(e.target.value)} className="border rounded-lg px-3 py-1.5 text-sm w-48" />
        {selected.size > 0 && (
          <div className="flex gap-2">
            {batchActions.map(({ label, action }) => (
              <button key={action} onClick={() => handleBatchAction(action)} className="text-xs border rounded px-2 py-1 hover:bg-gray-50">
                {label} ({selected.size})
              </button>
            ))}
          </div>
        )}
      </div>
      <table className="w-full text-sm">
        <thead>
          <tr className="border-b text-xs text-gray-500 uppercase">
            <th className="text-left p-3 w-8"><input type="checkbox" checked={selected.size === filtered.length && filtered.length > 0} onChange={toggleAll} /></th>
            <th className="text-left p-3">状态</th>
            <th className="text-left p-3">任务名称</th>
            <th className="text-left p-3">关联流</th>
            <th className="text-left p-3">规则</th>
            <th className="text-left p-3">抽帧数</th>
            <th className="text-left p-3">操作</th>
          </tr>
        </thead>
        <tbody>
          {filtered.map((t) => (
            <tr key={t.id} className="border-b hover:bg-gray-50">
              <td className="p-3"><input type="checkbox" checked={selected.has(t.id)} onChange={() => toggle(t.id)} /></td>
              <td className="p-3"><span className={`px-1.5 py-0.5 rounded text-xs font-medium ${statusStyle[t.status]}`}>{statusLabel[t.status]}</span></td>
              <td className="p-3 font-medium">{t.name}</td>
              <td className="p-3 text-gray-600">{t.stream_name}</td>
              <td className="p-3 text-gray-500">{t.rules?.[0]?.type || "-"}</td>
              <td className="p-3">{t.frames_extracted?.toLocaleString() || "0"}</td>
              <td className="p-3 space-x-2">
                <Link to={`/tasks/${t.id}`} className="text-gray-500 hover:underline">查看</Link>
                {t.status === "Created" && <button onClick={() => handleAction(t.id, "start")} className="text-brand hover:underline">启动</button>}
                {t.status === "Running" && <button onClick={() => handleAction(t.id, "pause")} className="text-yellow-600 hover:underline">暂停</button>}
                {t.status === "Paused" && <button onClick={() => handleAction(t.id, "resume")} className="text-brand hover:underline">恢复</button>}
                {(t.status === "Running" || t.status === "Paused") && <button onClick={() => handleAction(t.id, "stop")} className="text-error hover:underline">停止</button>}
                {t.status !== "Running" && t.status !== "Paused" && <button onClick={() => handleAction(t.id, "delete")} className="text-error hover:underline">删除</button>}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  )
}
