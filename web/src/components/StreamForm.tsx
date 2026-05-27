import { useState } from "react"
import { streamsApi } from "@/api/streams"
import type { StreamInfo, StreamConfig } from "@/types/stream"

const SOURCE_TYPES = ["", "rtsp", "rtmp", "hls", "file"] as const

interface StreamFormProps {
  stream?: StreamInfo | null
  onClose: () => void
  onSave: () => void
}

export function StreamForm({ stream, onClose, onSave }: StreamFormProps) {
  const [name, setName] = useState(stream?.name ?? "")
  const [url, setUrl] = useState(stream?.source_url ?? "")
  const [sourceType, setSourceType] = useState(stream?.source_type ?? "")
  const [tags, setTags] = useState(
    stream?.tags ? Object.entries(stream.tags).map(([k, v]) => `${k}:${v}`).join(", ") : ""
  )
  const [description, setDescription] = useState(stream?.description ?? "")
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState("")

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    setSaving(true)
    setError("")
    try {
      const tagMap: Record<string, string> = {}
      tags.split(",").filter(Boolean).forEach((t) => {
        const [k, ...vs] = t.trim().split(":")
        if (k) tagMap[k] = vs.join(":") || ""
      })
      const config: StreamConfig = {
        name, description, tags: tagMap, source_url: url,
        source_type: sourceType || "rtsp",
        extract_interval_seconds: 5, jpeg_quality: 85, ffmpeg_threads: 1, rtsp_transport: "tcp",
      }
      if (stream) await streamsApi.update(stream.id, config)
      else await streamsApi.create(config)
      onSave()
    } catch (err) {
      setError(err instanceof Error ? err.message : "保存失败")
    } finally {
      setSaving(false)
    }
  }

  return (
    <div className="fixed inset-0 bg-black/40 flex items-center justify-center z-50" onClick={onClose}>
      <div className="bg-white rounded-xl p-6 w-full max-w-lg shadow-xl" onClick={(e) => e.stopPropagation()}>
        <h2 className="text-lg font-bold mb-4">{stream ? "编辑流" : "新建流"}</h2>
        <form onSubmit={handleSubmit} className="space-y-4">
          <div>
            <label className="text-sm font-medium block mb-1">名称</label>
            <input required value={name} onChange={(e) => setName(e.target.value)} className="border rounded-lg px-3 py-1.5 w-full text-sm" />
          </div>
          <div>
            <label className="text-sm font-medium block mb-1">URL</label>
            <input required value={url} onChange={(e) => setUrl(e.target.value)} className="border rounded-lg px-3 py-1.5 w-full text-sm" placeholder="rtsp://..." />
          </div>
          <div className="grid grid-cols-2 gap-4">
            <div>
              <label className="text-sm font-medium block mb-1">类型</label>
              <select value={sourceType} onChange={(e) => setSourceType(e.target.value)} className="border rounded-lg px-3 py-1.5 w-full text-sm">
                {SOURCE_TYPES.map((t) => <option key={t} value={t}>{t || "自动检测"}</option>)}
              </select>
            </div>
            <div>
              <label className="text-sm font-medium block mb-1">标签</label>
              <input value={tags} onChange={(e) => setTags(e.target.value)} className="border rounded-lg px-3 py-1.5 w-full text-sm" placeholder="key:val, key2:val2" />
            </div>
          </div>
          <div>
            <label className="text-sm font-medium block mb-1">备注</label>
            <textarea value={description} onChange={(e) => setDescription(e.target.value)} className="border rounded-lg px-3 py-1.5 w-full text-sm" rows={2} />
          </div>
          {error && <div className="text-sm text-error bg-red-50 border border-red-200 rounded-lg p-2">{error}</div>}
          <div className="flex justify-end gap-3 pt-2">
            <button type="button" onClick={onClose} className="px-4 py-1.5 text-sm border rounded-lg hover:bg-gray-50">取消</button>
            <button type="submit" disabled={saving} className="px-4 py-1.5 text-sm bg-brand text-white rounded-lg hover:bg-blue-700 disabled:opacity-50">
              {saving ? "保存中..." : stream ? "保存" : "创建"}
            </button>
          </div>
        </form>
      </div>
    </div>
  )
}
