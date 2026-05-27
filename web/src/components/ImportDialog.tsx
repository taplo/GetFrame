import { useState, useRef } from "react"
import { streamsApi } from "@/api/streams"
import { tasksApi } from "@/api/tasks"
import type { StreamConfig } from "@/types/stream"

interface ImportDialogProps {
  type: "streams" | "tasks"
  onClose: () => void
  onImport: () => void
}

export function ImportDialog({ type, onClose, onImport }: ImportDialogProps) {
  const fileRef = useRef<HTMLInputElement>(null)
  const [results, setResults] = useState<{ success: number; errors: string[] } | null>(null)
  const [importing, setImporting] = useState(false)

  const handleFile = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0]
    if (!file) return
    setImporting(true)
    const text = await file.text()
    const lines = text.split("\n").filter(Boolean)
    const headers = lines[0].split(",").map((h) => h.trim())

    const streamNameToId: Record<string, string> = {}
    if (type === "tasks") {
      const { streams } = await streamsApi.list()
      for (const s of streams) streamNameToId[s.name] = s.id
    }

    let success = 0
    const errors: string[] = []

    for (let i = 1; i < lines.length; i++) {
      try {
        const vals = lines[i].split(",").map((v) => v.trim().replace(/^"|"$/g, ""))
        const row = Object.fromEntries(headers.map((h, j) => [h, vals[j] || ""]))
        if (type === "streams") {
          if (!row.name || !row.url) { errors.push(`行 ${i + 1}: name 和 url 为必填`); continue }
          const tagMap: Record<string, string> = {}
          if (row.tags) row.tags.split(";").filter(Boolean).forEach((t: string) => {
            const [k, ...vs] = t.split(":")
            if (k) tagMap[k.trim()] = vs.join(":").trim()
          })
          const config: StreamConfig = {
            name: row.name, description: row.description || "", tags: tagMap,
            source_url: row.url, source_type: row.type || "rtsp",
            extract_interval_seconds: 5, jpeg_quality: 85, ffmpeg_threads: 1, rtsp_transport: "tcp",
          }
          await streamsApi.create(config)
        } else {
          if (!row.name || !row.stream_name) { errors.push(`行 ${i + 1}: name 和 stream_name 为必填`); continue }
          const streamId = streamNameToId[row.stream_name]
          if (!streamId) { errors.push(`行 ${i + 1}: 未找到流 "${row.stream_name}"`); continue }
          await tasksApi.create({ name: row.name, stream_id: streamId, rules: [] })
        }
        success++
      } catch (err) {
        errors.push(`行 ${i + 1}: ${err instanceof Error ? err.message : "导入失败"}`)
      }
    }
    setResults({ success, errors })
    setImporting(false)
  }

  const downloadTemplate = () => {
    const template = type === "streams"
      ? 'name,url,type,tags,description\n"流名称","rtsp://...","rtsp","标签1:值1","备注"'
      : 'name,stream_name,rule_type,rule_params,description\n"任务名称","流名称","interval",{"interval_seconds":5},"备注"'
    const blob = new Blob(["\ufeff" + template], { type: "text/csv;charset=utf-8" })
    const url = URL.createObjectURL(blob)
    const a = document.createElement("a")
    a.href = url; a.download = type === "streams" ? "streams_import_template.csv" : "tasks_import_template.csv"
    a.click()
    URL.revokeObjectURL(url)
  }

  return (
    <div className="fixed inset-0 bg-black/40 flex items-center justify-center z-50" onClick={onClose}>
      <div className="bg-white rounded-xl p-6 w-full max-w-lg shadow-xl" onClick={(e) => e.stopPropagation()}>
        <h2 className="text-lg font-bold mb-4">批量导入 {type === "streams" ? "流" : "任务"}</h2>
        {!results ? (
          <div className="space-y-4">
            <div className="flex gap-3 items-center">
              <button onClick={downloadTemplate} className="px-3 py-1.5 text-sm border rounded-lg hover:bg-gray-50">下载 CSV 模板</button>
              <span className="text-gray-400 text-sm">或</span>
              <label className="px-3 py-1.5 text-sm bg-brand text-white rounded-lg cursor-pointer hover:bg-blue-700">
                {importing ? "导入中..." : "选择文件上传"}
                <input ref={fileRef} type="file" accept=".csv" hidden onChange={handleFile} disabled={importing} />
              </label>
            </div>
            <p className="text-xs text-gray-500">支持 .csv 格式，UTF-8 编码。第一行为表头。</p>
          </div>
        ) : (
          <div className="space-y-3">
            <div className="bg-green-50 border border-green-200 rounded-lg p-3 text-sm text-green-700">
              ✓ {results.success} 条导入成功
            </div>
            {results.errors.length > 0 && (
              <div className="bg-red-50 border border-red-200 rounded-lg p-3 text-sm text-red-700 max-h-40 overflow-auto">
                {results.errors.map((e, i) => <div key={i}>{e}</div>)}
              </div>
            )}
            <div className="flex justify-end gap-3 pt-2">
              <button onClick={() => { setResults(null); onImport(); onClose() }} className="px-4 py-1.5 text-sm border rounded-lg hover:bg-gray-50">完成</button>
            </div>
          </div>
        )}
      </div>
    </div>
  )
}
