import { useState } from "react"
import { streamsApi } from "@/api/streams"
import type { StreamInfo } from "@/types/stream"

interface StreamTableProps {
  streams: StreamInfo[]
  onEdit: (s: StreamInfo) => void
  onRefresh: () => void
}

export function StreamTable({ streams, onEdit, onRefresh }: StreamTableProps) {
  const [selected, setSelected] = useState<Set<string>>(new Set())
  const [search, setSearch] = useState("")

  const filtered = search
    ? streams.filter((s) => s.name.toLowerCase().includes(search.toLowerCase()) || s.source_url.toLowerCase().includes(search.toLowerCase()))
    : streams

  const toggle = (id: string) => {
    const next = new Set(selected)
    if (next.has(id)) next.delete(id); else next.add(id)
    setSelected(next)
  }

  const toggleAll = () => {
    if (selected.size === streams.length) setSelected(new Set())
    else setSelected(new Set(streams.map((s) => s.id)))
  }

  const handleDelete = async (id: string) => {
    if (!confirm("确定删除此流？")) return
    await streamsApi.delete(id)
    onRefresh()
  }

  const handleBatchDelete = async () => {
    if (!confirm(`确定删除选中的 ${selected.size} 个流？`)) return
    for (const id of selected) await streamsApi.delete(id)
    setSelected(new Set())
    onRefresh()
  }

  return (
    <div className="bg-white border rounded-xl shadow-sm overflow-hidden">
      <div className="p-4 border-b flex items-center justify-between gap-4">
        <input placeholder="搜索流..." value={search} onChange={(e) => setSearch(e.target.value)} className="border rounded-lg px-3 py-1.5 text-sm w-48" />
        {selected.size > 0 && (
          <button onClick={handleBatchDelete} className="text-sm text-error hover:underline">
            删除选中 ({selected.size})
          </button>
        )}
      </div>
      <table className="w-full text-sm">
        <thead>
          <tr className="border-b text-xs text-gray-500 uppercase">
            <th className="text-left p-3 w-8"><input type="checkbox" checked={selected.size === filtered.length && filtered.length > 0} onChange={toggleAll} /></th>
            <th className="text-left p-3">状态</th>
            <th className="text-left p-3">名称</th>
            <th className="text-left p-3">URL</th>
            <th className="text-left p-3">类型</th>
            <th className="text-left p-3">标签</th>
            <th className="text-left p-3">操作</th>
          </tr>
        </thead>
        <tbody>
          {filtered.map((s) => (
            <tr key={s.id} className="border-b hover:bg-gray-50">
              <td className="p-3"><input type="checkbox" checked={selected.has(s.id)} onChange={() => toggle(s.id)} /></td>
              <td className="p-3"><span className={s.status === "online" ? "text-success" : s.status.startsWith("error") ? "text-error" : "text-gray-500"}>● {s.status === "online" ? "在线" : s.status.startsWith("error") ? "异常" : "离线"}</span></td>
              <td className="p-3 font-medium">{s.name}</td>
              <td className="p-3 text-gray-500 truncate max-w-48">{s.source_url}</td>
              <td className="p-3">{s.source_type}</td>
              <td className="p-3">{Object.entries(s.tags || {}).map(([k, v]) => (
                <span key={k} className="inline-block bg-gray-100 rounded px-1.5 py-0.5 text-xs mr-1">{k}:{v}</span>
              ))}</td>
              <td className="p-3 space-x-2">
                <button onClick={() => onEdit(s)} className="text-brand hover:underline">编辑</button>
                <button onClick={() => handleDelete(s.id)} className="text-error hover:underline">删除</button>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  )
}
