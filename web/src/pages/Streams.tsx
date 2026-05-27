import { useState, useEffect, useCallback } from "react"
import { streamsApi } from "@/api/streams"
import { StreamTable } from "@/components/StreamTable"
import { StreamForm } from "@/components/StreamForm"
import { ImportDialog } from "@/components/ImportDialog"
import type { StreamInfo } from "@/types/stream"

export function StreamsPage() {
  const [streams, setStreams] = useState<StreamInfo[]>([])
  const [editing, setEditing] = useState<StreamInfo | null>(null)
  const [showForm, setShowForm] = useState(false)
  const [showImport, setShowImport] = useState(false)

  const load = useCallback(() => {
    streamsApi.list().then((res) => setStreams(res.streams)).catch(() => {})
  }, [])

  useEffect(() => { load() }, [load])

  const handleExport = () => {
    const csv = ["name,url,type,tags,description",
      ...streams.map((s) =>
        [s.name, s.source_url, s.source_type,
          Object.entries(s.tags || {}).map(([k, v]) => `${k}:${v}`).join(";"),
          s.description || ""
        ].map((v) => `"${v.replace(/"/g, '""')}"`).join(",")
      )
    ].join("\n")
    const blob = new Blob(["\ufeff" + csv], { type: "text/csv;charset=utf-8" })
    const url = URL.createObjectURL(blob)
    const a = document.createElement("a")
    a.href = url; a.download = "streams.csv"; a.click()
    URL.revokeObjectURL(url)
  }

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold">流管理</h1>
        <div className="flex gap-2">
          <button onClick={handleExport} className="px-3 py-1.5 text-sm border rounded-lg hover:bg-gray-50">导出 CSV</button>
          <button onClick={() => setShowImport(true)} className="px-3 py-1.5 text-sm border rounded-lg hover:bg-gray-50">导入 CSV</button>
          <button onClick={() => { setEditing(null); setShowForm(true) }} className="px-3 py-1.5 text-sm bg-brand text-white rounded-lg hover:bg-blue-700">+ 新建流</button>
        </div>
      </div>
      <StreamTable streams={streams} onEdit={(s) => { setEditing(s); setShowForm(true) }} onRefresh={load} />
      {showForm && <StreamForm stream={editing} onClose={() => setShowForm(false)} onSave={() => { setShowForm(false); load() }} />}
      {showImport && <ImportDialog type="streams" onClose={() => setShowImport(false)} onImport={load} />}
    </div>
  )
}
