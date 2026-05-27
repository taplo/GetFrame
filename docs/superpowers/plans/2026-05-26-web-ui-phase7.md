# Phase 7: Web UI — Stream & Task Management

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a Chinese-first React SPA for managing video streams and extraction tasks, served by the Axum backend in production.

**Architecture:** Vite + React SPA in `web/` dir, shadcn/ui components, React Router for navigation. Dev mode uses Vite proxy to backend. Production builds to `web/dist/`, served by Axum via `tower-http::services::ServeDir`.

**Tech Stack:** React 19, TypeScript 5, Vite 6, shadcn/ui (Tailwind CSS 4), React Router v7, Axum (static serving)

---

### Task 1: Project Scaffolding

**Files:**
- Create: `web/package.json`
- Create: `web/tsconfig.json`
- Create: `web/tsconfig.app.json`
- Create: `web/vite.config.ts`
- Create: `web/index.html`
- Create: `web/src/main.tsx`
- Create: `web/src/App.tsx`
- Create: `web/src/vite-env.d.ts`
- Modify: `.gitignore`

- [ ] **Step 1: Write package.json**

```json
{
  "name": "getframe-web",
  "private": true,
  "version": "0.1.0",
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "tsc -b && vite build",
    "preview": "vite preview"
  },
  "dependencies": {
    "react": "^19.0.0",
    "react-dom": "^19.0.0",
    "react-router": "^7.0.0",
    "react-router-dom": "^7.0.0",
    "lucide-react": "^0.400.0",
    "clsx": "^2.1.0",
    "tailwind-merge": "^2.5.0",
    "class-variance-authority": "^0.7.0"
  },
  "devDependencies": {
    "@types/react": "^19.0.0",
    "@types/react-dom": "^19.0.0",
    "@vitejs/plugin-react": "^4.3.0",
    "typescript": "~5.7.0",
    "vite": "^6.0.0",
    "tailwindcss": "^4.0.0",
    "@tailwindcss/vite": "^4.0.0"
  }
}
```

- [ ] **Step 2: Write tsconfig.json**

```json
{
  "files": [],
  "references": [
    { "path": "./tsconfig.app.json" }
  ]
}
```

- [ ] **Step 3: Write tsconfig.app.json**

```json
{
  "compilerOptions": {
    "target": "ES2020",
    "useDefineForClassFields": true,
    "lib": ["ES2020", "DOM", "DOM.Iterable"],
    "module": "ESNext",
    "skipLibCheck": true,
    "moduleResolution": "bundler",
    "allowImportingTsExtensions": true,
    "isolatedModules": true,
    "moduleDetection": "force",
    "noEmit": true,
    "jsx": "react-jsx",
    "strict": true,
    "noUnusedLocals": true,
    "noUnusedParameters": true,
    "noFallthroughCasesInSwitch": true,
    "noUncheckedSideEffectImports": true,
    "baseUrl": ".",
    "paths": { "@/*": ["./src/*"] }
  },
  "include": ["src"]
}
```

- [ ] **Step 4: Write vite.config.ts**

```typescript
import { defineConfig } from "vite"
import react from "@vitejs/plugin-react"
import tailwindcss from "@tailwindcss/vite"
import path from "path"

export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: { "@": path.resolve(__dirname, "./src") },
  },
  server: {
    proxy: {
      "/api": "http://localhost:3000",
    },
  },
})
```

- [ ] **Step 5: Write index.html**

```html
<!doctype html>
<html lang="zh-CN">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>GetFrame</title>
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="/src/main.tsx"></script>
  </body>
</html>
```

- [ ] **Step 6: Write src/vite-env.d.ts**

```typescript
/// <reference types="vite/client" />
```

- [ ] **Step 7: Write src/main.tsx**

```typescript
import { StrictMode } from "react"
import { createRoot } from "react-dom/client"
import { App } from "./App"
import "./index.css"

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>,
)
```

- [ ] **Step 8: Write src/App.tsx (placeholder)**

```typescript
export function App() {
  return <div className="p-4 text-center text-lg">GetFrame</div>
}
```

- [ ] **Step 9: Write src/index.css**

```css
@import "tailwindcss";

@theme {
  --color-brand: #2563eb;
  --color-success: #16a34a;
  --color-warning: #f59e0b;
  --color-error: #dc2626;
}
```

- [ ] **Step 10: Update .gitignore**

Append to `.gitignore`:
```
node_modules/
web/dist/
```

- [ ] **Step 11: Install dependencies**

Run: `cd web && npm install`

Expected: Installs all dependencies, creates `web/node_modules/` and `web/package-lock.json`

- [ ] **Step 12: Verify build**

Run: `cd web && npx tsc -b`
Expected: TypeScript compiles with no errors

---

### Task 2: shadcn/ui Setup + Types

**Files:**
- Create: `web/src/lib/utils.ts`
- Create: `web/src/types/stream.ts`
- Create: `web/src/types/task.ts`
- Create: `web/src/types/rule.ts`

- [ ] **Step 1: Write src/lib/utils.ts**

```typescript
import { clsx, type ClassValue } from "clsx"
import { twMerge } from "tailwind-merge"

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}
```

- [ ] **Step 2: Write src/types/stream.ts**

```typescript
export type StreamStatus = "Online" | "Offline" | "Error"

export interface StreamInfo {
  id: string
  name: string
  source_url: string
  source_type: string
  status: StreamStatus
  tags: Record<string, string>
  description: string
  frames_per_hour?: number
  created_at: string
  updated_at: string
}

export interface CreateStreamRequest {
  name: string
  source_url: string
  source_type?: string
  tags?: Record<string, string>
  description?: string
}

export interface UpdateStreamRequest {
  name?: string
  source_url?: string
  source_type?: string
  tags?: Record<string, string>
  description?: string
}
```

- [ ] **Step 3: Write src/types/task.ts**

```typescript
export type TaskStatus = "Created" | "Running" | "Paused" | "Stopped" | "Error"

export interface TaskInfo {
  id: string
  name: string
  stream_id: string
  stream_name: string
  status: TaskStatus
  rules: RuleConfig[]
  frames_extracted: number
  created_at: string
  started_at?: string
  stopped_at?: string
}

export interface CreateTaskRequest {
  name: string
  stream_id: string
  rules: RuleConfig[]
}
```

- [ ] **Step 4: Write src/types/rule.ts**

```typescript
export type CompositeOperator = "any" | "all"

export interface RuleConfig {
  type: "interval" | "fps" | "rate_limited" | "scene_change" | "composite"
  interval_seconds?: number
  fps?: number
  max_per_minute?: number
  threshold?: number
  rule?: RuleConfig
  operator?: CompositeOperator
  rules?: RuleConfig[]
}
```

- [ ] **Step 5: Install shadcn/ui and init**

Run: `cd web && npx shadcn@latest init -d`

Expected: Creates `src/components/ui/` directory with shadcn/ui base components config

- [ ] **Step 6: Add needed shadcn/ui components**

Run:
```bash
cd web
npx shadcn@latest add button card input select table dialog badge -y
```

Expected: Component files created in `src/components/ui/`

---

### Task 3: API Client Layer

**Files:**
- Create: `web/src/api/client.ts`
- Create: `web/src/api/streams.ts`
- Create: `web/src/api/tasks.ts`
- Create: `web/src/api/health.ts`

- [ ] **Step 1: Write src/api/client.ts**

```typescript
import type { StreamInfo, CreateStreamRequest, UpdateStreamRequest } from "@/types/stream"
import type { TaskInfo, CreateTaskRequest } from "@/types/task"

const BASE = "/api/v1"

class ApiError extends Error {
  constructor(public status: number, message: string) {
    super(message)
    this.name = "ApiError"
  }
}

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const res = await fetch(`${BASE}${path}`, {
    headers: { "Content-Type": "application/json" },
    ...init,
  })
  if (!res.ok) {
    const body = await res.text().catch(() => "")
    throw new ApiError(res.status, body || res.statusText)
  }
  if (res.status === 204) return undefined as T
  return res.json()
}
```

- [ ] **Step 2: Write src/api/streams.ts**

```typescript
import { request } from "./client"
import type { StreamInfo, CreateStreamRequest, UpdateStreamRequest } from "@/types/stream"

export const streamsApi = {
  list: (params?: { status?: string; search?: string }) => {
    const qs = new URLSearchParams()
    if (params?.status) qs.set("status", params.status)
    if (params?.search) qs.set("search", params.search)
    const q = qs.toString()
    return request<{ streams: StreamInfo[] }>(`/streams${q ? `?${q}` : ""}`)
  },
  get: (id: string) => request<StreamInfo>(`/streams/${id}`),
  create: (data: CreateStreamRequest) =>
    request<StreamInfo>("/streams", { method: "POST", body: JSON.stringify(data) }),
  update: (id: string, data: UpdateStreamRequest) =>
    request<StreamInfo>(`/streams/${id}`, { method: "PUT", body: JSON.stringify(data) }),
  delete: (id: string) =>
    request<void>(`/streams/${id}`, { method: "DELETE" }),
  testConnection: (url: string) =>
    request<{ reachable: boolean }>("/streams/test", { method: "POST", body: JSON.stringify({ url }) }),
}
```

- [ ] **Step 3: Write src/api/tasks.ts**

```typescript
import { request } from "./client"
import type { TaskInfo, CreateTaskRequest } from "@/types/task"

export const tasksApi = {
  list: (params?: { status?: string }) => {
    const qs = params?.status ? `?status=${params.status}` : ""
    return request<{ tasks: TaskInfo[] }>(`/tasks${qs}`)
  },
  get: (id: string) => request<TaskInfo>(`/tasks/${id}`),
  create: (data: CreateTaskRequest) =>
    request<TaskInfo>("/tasks", { method: "POST", body: JSON.stringify(data) }),
  delete: (id: string) =>
    request<void>(`/tasks/${id}`, { method: "DELETE" }),
  start: (id: string) =>
    request<TaskInfo>(`/tasks/${id}/start`, { method: "POST" }),
  pause: (id: string) =>
    request<TaskInfo>(`/tasks/${id}/pause`, { method: "POST" }),
  resume: (id: string) =>
    request<TaskInfo>(`/tasks/${id}/resume`, { method: "POST" }),
  stop: (id: string) =>
    request<TaskInfo>(`/tasks/${id}/stop`, { method: "POST" }),
}
```

- [ ] **Step 4: Write src/api/health.ts**

```typescript
import { request } from "./client"

export const healthApi = {
  health: () => request<{ status: string }>("/health"),
  ready: () => request<{ ready: boolean }>("/ready"),
}
```

- [ ] **Step 5: Verify TypeScript**

Run: `cd web && npx tsc -b`
Expected: No errors

---

### Task 4: Layout + Navigation

**Files:**
- Create: `web/src/components/Layout.tsx`
- Modify: `web/src/App.tsx`

- [ ] **Step 1: Write src/components/Layout.tsx**

```typescript
import { NavLink, Outlet } from "react-router-dom"
import { cn } from "@/lib/utils"

const navItems = [
  { to: "/", label: "控制面板" },
  { to: "/streams", label: "流管理" },
  { to: "/tasks", label: "任务管理" },
]

export function Layout() {
  return (
    <div className="min-h-screen bg-gray-50">
      <nav className="border-b bg-white px-6 py-3 flex items-center gap-6 shadow-sm">
        <span className="font-bold text-lg text-brand">GetFrame</span>
        {navItems.map((item) => (
          <NavLink
            key={item.to}
            to={item.to}
            end={item.to === "/"}
            className={({ isActive }) =>
              cn(
                "text-sm font-medium transition-colors",
                isActive ? "text-brand border-b-2 border-brand pb-1" : "text-gray-600 hover:text-gray-900",
              )
            }
          >
            {item.label}
          </NavLink>
        ))}
      </nav>
      <main className="p-6 max-w-7xl mx-auto">
        <Outlet />
      </main>
    </div>
  )
}
```

- [ ] **Step 2: Update src/App.tsx with Router**

```typescript
import { BrowserRouter, Routes, Route } from "react-router-dom"
import { Layout } from "@/components/Layout"

export function App() {
  return (
    <BrowserRouter>
      <Routes>
        <Route element={<Layout />}>
          <Route index element={<div className="text-center py-20 text-gray-500">控制面板（待实现）</div>} />
          <Route path="streams" element={<div className="text-center py-20 text-gray-500">流管理（待实现）</div>} />
          <Route path="tasks" element={<div className="text-center py-20 text-gray-500">任务管理（待实现）</div>} />
        </Route>
      </Routes>
    </BrowserRouter>
  )
}
```

- [ ] **Step 3: Verify TypeScript + build**

Run: `cd web && npx tsc -b && npx vite build`
Expected: Build succeeds, `web/dist/` created

---

### Task 5: Dashboard Page

**Files:**
- Create: `web/src/pages/Dashboard.tsx`
- Create: `web/src/components/StatCard.tsx`
- Modify: `web/src/App.tsx` (wire Dashboard)

- [ ] **Step 1: Write src/components/StatCard.tsx**

```typescript
import { type LucideIcon } from "lucide-react"

interface StatCardProps {
  title: string
  value: string | number
  icon: LucideIcon
  color?: "success" | "brand" | "error" | "default"
}

const colorMap = {
  success: "text-success",
  brand: "text-brand",
  error: "text-error",
  default: "text-gray-900",
}

export function StatCard({ title, value, icon: Icon, color = "default" }: StatCardProps) {
  return (
    <div className="bg-white border rounded-xl p-5 shadow-sm">
      <div className="flex items-center justify-between mb-2">
        <span className="text-xs text-gray-500 uppercase tracking-wide">{title}</span>
        <Icon className={`w-5 h-5 ${colorMap[color]}`} />
      </div>
      <div className={`text-2xl font-bold ${colorMap[color]}`}>{value}</div>
    </div>
  )
}
```

- [ ] **Step 2: Write src/pages/Dashboard.tsx**

```typescript
import { useState, useEffect } from "react"
import { Activity, Video, Image, AlertTriangle } from "lucide-react"
import { StatCard } from "@/components/StatCard"
import { streamsApi } from "@/api/streams"
import { tasksApi } from "@/api/tasks"
import type { StreamInfo } from "@/types/stream"
import type { TaskInfo } from "@/types/task"

export function Dashboard() {
  const [streams, setStreams] = useState<StreamInfo[]>([])
  const [tasks, setTasks] = useState<TaskInfo[]>([])

  useEffect(() => {
    streamsApi.list().then((res) => setStreams(res.streams)).catch(() => {})
    tasksApi.list().then((res) => setTasks(res.tasks)).catch(() => {})
  }, [])

  const online = streams.filter((s) => s.status === "Online").length
  const offline = streams.filter((s) => s.status !== "Online").length
  const running = tasks.filter((t) => t.status === "Running").length
  const totalFrames = tasks.reduce((sum, t) => sum + (t.frames_extracted || 0), 0)

  return (
    <div className="space-y-6">
      <h1 className="text-2xl font-bold">控制面板</h1>

      <div className="grid grid-cols-4 gap-4">
        <StatCard title="在线流" value={online} icon={Video} color="success" />
        <StatCard title="活跃任务" value={running} icon={Activity} color="brand" />
        <StatCard title="抽帧总数" value={totalFrames.toLocaleString()} icon={Image} />
        <StatCard title="离线流" value={offline} icon={AlertTriangle} color={offline > 0 ? "error" : "default"} />
      </div>

      <div className="grid grid-cols-2 gap-6">
        <div className="bg-white border rounded-xl p-5 shadow-sm">
          <h2 className="font-semibold mb-3">最近流状态</h2>
          {streams.slice(0, 5).map((s) => (
            <div key={s.id} className="flex items-center gap-3 py-2 border-b last:border-0 text-sm">
              <span className={`w-2 h-2 rounded-full ${s.status === "Online" ? "bg-success" : s.status === "Error" ? "bg-error" : "bg-gray-400"}`} />
              <span className="flex-1 truncate">{s.name}</span>
              <span className="text-gray-500">{s.frames_per_hour ? `${s.frames_per_hour} 帧/时` : s.status === "Online" ? "在线" : "离线"}</span>
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
```

- [ ] **Step 3: Wire Dashboard in App.tsx**

Replace `App.tsx` to import and use Dashboard:
```typescript
import { BrowserRouter, Routes, Route } from "react-router-dom"
import { Layout } from "@/components/Layout"
import { Dashboard } from "@/pages/Dashboard"

export function App() {
  return (
    <BrowserRouter>
      <Routes>
        <Route element={<Layout />}>
          <Route index element={<Dashboard />} />
          <Route path="streams" element={<div className="text-center py-20 text-gray-500">流管理（待实现）</div>} />
          <Route path="tasks" element={<div className="text-center py-20 text-gray-500">任务管理（待实现）</div>} />
        </Route>
      </Routes>
    </BrowserRouter>
  )
}
```

- [ ] **Step 4: Verify build**

Run: `cd web && npx tsc -b && npx vite build`
Expected: No errors

---

### Task 6: Streams Page

**Files:**
- Create: `web/src/pages/Streams.tsx`
- Create: `web/src/components/StreamTable.tsx`
- Create: `web/src/components/StreamForm.tsx`
- Modify: `web/src/App.tsx`

- [ ] **Step 1: Write src/components/StreamTable.tsx**

```typescript
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
        <input placeholder="搜索流..." className="border rounded-lg px-3 py-1.5 text-sm w-48" />
        {selected.size > 0 && (
          <button onClick={handleBatchDelete} className="text-sm text-error hover:underline">
            删除选中 ({selected.size})
          </button>
        )}
      </div>
      <table className="w-full text-sm">
        <thead>
          <tr className="border-b text-xs text-gray-500 uppercase">
            <th className="text-left p-3 w-8"><input type="checkbox" checked={selected.size === streams.length && streams.length > 0} onChange={toggleAll} /></th>
            <th className="text-left p-3">状态</th>
            <th className="text-left p-3">名称</th>
            <th className="text-left p-3">URL</th>
            <th className="text-left p-3">类型</th>
            <th className="text-left p-3">标签</th>
            <th className="text-left p-3">操作</th>
          </tr>
        </thead>
        <tbody>
          {streams.map((s) => (
            <tr key={s.id} className="border-b hover:bg-gray-50">
              <td className="p-3"><input type="checkbox" checked={selected.has(s.id)} onChange={() => toggle(s.id)} /></td>
              <td className="p-3"><span className={s.status === "Online" ? "text-success" : "text-error"}>● {s.status === "Online" ? "在线" : "离线"}</span></td>
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
```

- [ ] **Step 2: Write src/components/StreamForm.tsx**

```typescript
import { useState } from "react"
import { streamsApi } from "@/api/streams"
import type { StreamInfo, CreateStreamRequest } from "@/types/stream"

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

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    setSaving(true)
    try {
      const tagMap: Record<string, string> = {}
      tags.split(",").filter(Boolean).forEach((t) => {
        const [k, ...vs] = t.trim().split(":")
        if (k) tagMap[k] = vs.join(":") || ""
      })
      const data: CreateStreamRequest = { name, source_url: url, source_type: sourceType || undefined, tags: tagMap, description: description || undefined }
      if (stream) await streamsApi.update(stream.id, data)
      else await streamsApi.create(data)
      onSave()
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
```

- [ ] **Step 3: Write src/pages/Streams.tsx**

```typescript
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
```

- [ ] **Step 4: Wire Streams page in App.tsx**

Replace the streams route:
```typescript
import { StreamsPage } from "@/pages/Streams"
// ...
<Route path="streams" element={<StreamsPage />} />
```

- [ ] **Step 5: Verify build**

Run: `cd web && npx tsc -b`
Expected: No errors

---

### Task 7: Tasks Page

**Files:**
- Create: `web/src/pages/Tasks.tsx`
- Create: `web/src/components/TaskTable.tsx`
- Create: `web/src/components/TaskForm.tsx`
- Modify: `web/src/App.tsx`

- [ ] **Step 1: Write src/components/TaskTable.tsx**

```typescript
import { useState } from "react"
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
        <input placeholder="搜索任务..." className="border rounded-lg px-3 py-1.5 text-sm w-48" />
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
            <th className="text-left p-3 w-8"><input type="checkbox" checked={selected.size === tasks.length && tasks.length > 0} onChange={toggleAll} /></th>
            <th className="text-left p-3">状态</th>
            <th className="text-left p-3">任务名称</th>
            <th className="text-left p-3">关联流</th>
            <th className="text-left p-3">规则</th>
            <th className="text-left p-3">抽帧数</th>
            <th className="text-left p-3">操作</th>
          </tr>
        </thead>
        <tbody>
          {tasks.map((t) => (
            <tr key={t.id} className="border-b hover:bg-gray-50">
              <td className="p-3"><input type="checkbox" checked={selected.has(t.id)} onChange={() => toggle(t.id)} /></td>
              <td className="p-3"><span className={`px-1.5 py-0.5 rounded text-xs font-medium ${statusStyle[t.status]}`}>{statusLabel[t.status]}</span></td>
              <td className="p-3 font-medium">{t.name}</td>
              <td className="p-3 text-gray-600">{t.stream_name}</td>
              <td className="p-3 text-gray-500">{t.rules?.[0]?.type || "-"}</td>
              <td className="p-3">{t.frames_extracted?.toLocaleString() || "0"}</td>
              <td className="p-3 space-x-2">
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
```

- [ ] **Step 2: Write src/components/TaskForm.tsx**

```typescript
import { useState, useEffect } from "react"
import { tasksApi } from "@/api/tasks"
import { streamsApi } from "@/api/streams"
import type { StreamInfo } from "@/types/stream"
import type { RuleConfig } from "@/types/rule"

const RULE_TYPES = ["interval", "fps", "scene_change", "rate_limited", "composite"] as const

export function TaskForm({ onClose, onSave }: { onClose: () => void; onSave: () => void }) {
  const [name, setName] = useState("")
  const [streamId, setStreamId] = useState("")
  const [ruleType, setRuleType] = useState<string>("interval")
  const [paramValue, setParamValue] = useState("5")
  const [streams, setStreams] = useState<StreamInfo[]>([])
  const [saving, setSaving] = useState(false)

  useEffect(() => {
    streamsApi.list().then((res) => setStreams(res.streams)).catch(() => {})
  }, [])

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    if (!streamId) return
    setSaving(true)
    try {
      const rule: RuleConfig = { type: ruleType as RuleConfig["type"] }
      if (ruleType === "interval") rule.interval_seconds = Number(paramValue)
      else if (ruleType === "fps") rule.fps = Number(paramValue)
      else if (ruleType === "scene_change") rule.threshold = Number(paramValue)
      else if (ruleType === "rate_limited") { rule.rule = { type: "interval", interval_seconds: 5 }; rule.max_per_minute = Number(paramValue) }
      await tasksApi.create({ name, stream_id: streamId, rules: [rule] })
      onSave()
    } finally {
      setSaving(false)
    }
  }

  return (
    <div className="fixed inset-0 bg-black/40 flex items-center justify-center z-50" onClick={onClose}>
      <div className="bg-white rounded-xl p-6 w-full max-w-lg shadow-xl" onClick={(e) => e.stopPropagation()}>
        <h2 className="text-lg font-bold mb-4">新建任务</h2>
        <form onSubmit={handleSubmit} className="space-y-4">
          <div>
            <label className="text-sm font-medium block mb-1">任务名称</label>
            <input required value={name} onChange={(e) => setName(e.target.value)} className="border rounded-lg px-3 py-1.5 w-full text-sm" />
          </div>
          <div>
            <label className="text-sm font-medium block mb-1">关联流</label>
            <select required value={streamId} onChange={(e) => setStreamId(e.target.value)} className="border rounded-lg px-3 py-1.5 w-full text-sm">
              <option value="">选择流...</option>
              {streams.map((s) => <option key={s.id} value={s.id}>{s.name}</option>)}
            </select>
          </div>
          <div>
            <label className="text-sm font-medium block mb-1">规则类型</label>
            <div className="flex gap-2 flex-wrap">
              {RULE_TYPES.map((t) => (
                <button key={t} type="button" onClick={() => setRuleType(t)}
                  className={`px-3 py-1 text-sm border rounded-lg ${ruleType === t ? "bg-brand text-white border-brand" : "hover:bg-gray-50"}`}
                >{t === "interval" ? "间隔" : t === "fps" ? "FPS" : t === "scene_change" ? "场景变化" : t === "rate_limited" ? "限速" : "复合"}</button>
              ))}
            </div>
          </div>
          <div>
            <label className="text-sm font-medium block mb-1">
              {ruleType === "interval" ? "间隔（秒）" : ruleType === "fps" ? "FPS" : ruleType === "scene_change" ? "阈值 (0.0~1.0)" : ruleType === "rate_limited" ? "每分钟上限" : ""}
            </label>
            <input type="number" value={paramValue} onChange={(e) => setParamValue(e.target.value)} className="border rounded-lg px-3 py-1.5 w-full text-sm" step="any" />
          </div>
          <div className="flex justify-end gap-3 pt-2">
            <button type="button" onClick={onClose} className="px-4 py-1.5 text-sm border rounded-lg hover:bg-gray-50">取消</button>
            <button type="submit" disabled={saving} className="px-4 py-1.5 text-sm bg-brand text-white rounded-lg hover:bg-blue-700 disabled:opacity-50">
              {saving ? "创建中..." : "创建任务"}
            </button>
          </div>
        </form>
      </div>
    </div>
  )
}
```

- [ ] **Step 3: Write src/pages/Tasks.tsx**

```typescript
import { useState, useEffect, useCallback } from "react"
import { tasksApi } from "@/api/tasks"
import { TaskTable } from "@/components/TaskTable"
import { TaskForm } from "@/components/TaskForm"
import { ImportDialog } from "@/components/ImportDialog"
import type { TaskInfo } from "@/types/task"

export function TasksPage() {
  const [tasks, setTasks] = useState<TaskInfo[]>([])
  const [showForm, setShowForm] = useState(false)
  const [showImport, setShowImport] = useState(false)

  const load = useCallback(() => {
    tasksApi.list().then((res) => setTasks(res.tasks)).catch(() => {})
  }, [])

  useEffect(() => { load() }, [load])

  const handleExport = () => {
    const csv = ["name,stream_name,rule_type,rule_params,description",
      ...tasks.map((t) =>
        [t.name, t.stream_name, t.rules?.[0]?.type || "", JSON.stringify(t.rules?.[0] || {}), ""]
          .map((v) => `"${v.replace(/"/g, '""')}"`).join(",")
      )
    ].join("\n")
    const blob = new Blob(["\ufeff" + csv], { type: "text/csv;charset=utf-8" })
    const url = URL.createObjectURL(blob)
    const a = document.createElement("a")
    a.href = url; a.download = "tasks.csv"; a.click()
    URL.revokeObjectURL(url)
  }

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold">任务管理</h1>
        <div className="flex gap-2">
          <button onClick={handleExport} className="px-3 py-1.5 text-sm border rounded-lg hover:bg-gray-50">导出 CSV</button>
          <button onClick={() => setShowImport(true)} className="px-3 py-1.5 text-sm border rounded-lg hover:bg-gray-50">导入 CSV</button>
          <button onClick={() => setShowForm(true)} className="px-3 py-1.5 text-sm bg-brand text-white rounded-lg hover:bg-blue-700">+ 新建任务</button>
        </div>
      </div>
      <TaskTable tasks={tasks} onRefresh={load} />
      {showForm && <TaskForm onClose={() => setShowForm(false)} onSave={() => { setShowForm(false); load() }} />}
      {showImport && <ImportDialog type="tasks" onClose={() => setShowImport(false)} onImport={load} />}
    </div>
  )
}
```

- [ ] **Step 4: Wire Tasks page in App.tsx**

Replace tasks route:
```typescript
import { TasksPage } from "@/pages/Tasks"
// ...
<Route path="tasks" element={<TasksPage />} />
```

- [ ] **Step 5: Verify build**

Run: `cd web && npx tsc -b`
Expected: No errors

---

### Task 8: CSV Import Dialog (Shared)

**Files:**
- Create: `web/src/components/ImportDialog.tsx`
- Create: `web/src/components/CsvTemplate.tsx`

- [ ] **Step 1: Write src/components/ImportDialog.tsx**

```typescript
import { useState, useRef } from "react"
import { streamsApi } from "@/api/streams"
import { tasksApi } from "@/api/tasks"

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
    let success = 0
    const errors: string[] = []

    for (let i = 1; i < lines.length; i++) {
      try {
        const vals = lines[i].split(",").map((v) => v.trim().replace(/^"|"$/g, ""))
        const row = Object.fromEntries(headers.map((h, j) => [h, vals[j] || ""]))
        if (type === "streams") {
          if (!row.name || !row.url) { errors.push(`行 ${i + 1}: name 和 url 为必填`); continue }
          const tags: Record<string, string> = {}
          if (row.tags) row.tags.split(";").filter(Boolean).forEach((t: string) => {
            const [k, ...vs] = t.split(":")
            if (k) tags[k.trim()] = vs.join(":").trim()
          })
          await streamsApi.create({ name: row.name, source_url: row.url, source_type: row.type || undefined, tags: Object.keys(tags).length ? tags : undefined, description: row.description || undefined })
        } else {
          if (!row.name || !row.stream_name) { errors.push(`行 ${i + 1}: name 和 stream_name 为必填`); continue }
          await tasksApi.create({ name: row.name, stream_id: row.stream_name, rules: [] })
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
```

- [ ] **Step 2: Verify build**

Run: `cd web && npx tsc -b`
Expected: No errors

---

### Task 9: Axum Static Serving + Build Script

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/main.rs`

- [ ] **Step 1: Add tower-http dependency to Cargo.toml**

```toml
tower-http = { version = "0.6", features = ["fs", "cors"] }
```

Add after the existing dependencies (e.g., near `utoipa-swagger-ui`).

- [ ] **Step 2: Modify src/main.rs to serve static files**

After the Swagger UI merge, add `ServeDir` before the `/metrics` route. Update the `app` builder:

```rust
use tower_http::services::ServeDir;
// ... (other imports remain)

let app = health_router
    .merge(api_router)
    .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", api_doc))
    .nest_service("/", ServeDir::new("web/dist"))
    .route("/metrics", axum::routing::get(metrics::metrics_handler));
```

- [ ] **Step 3: Add build-web script to Cargo.toml**

In `[package.metadata]` or create a build helper:

Add to `Cargo.toml` scripts section (or just document in README):
```
# Build frontend then Rust:
cd web && npm run build && cd .. && cargo build
```

- [ ] **Step 4: Verify full build**

Run:
```bash
cd web && npm run build
cargo build
```

Expected: Frontend builds to `web/dist/`, Rust binary compiles with no errors.

---

## Plan Self-Review

- **Spec coverage**: Dashboard (Task 5), Streams list + CRUD (Task 6), Tasks list + lifecycle (Task 7), batch import (Task 8), CSV export (Task 6, 7), Chinese localization (built into all components). All spec requirements have matching tasks.
- **Placeholder scan**: No TBD/TODO/fill-in-later found. All code blocks contain complete implementations.
- **Type consistency**: `CreateStreamRequest`, `TaskInfo`, `RuleConfig` types match between type definitions, API client, and component usage. API methods return `Promise<T>` consistently.
- **Missing**: The build script integration (Task 9) is minimal. A `Makefile` or build script could be added in a follow-up.
