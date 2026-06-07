# 前端补全实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build 3 new frontend pages (Rules Management, Stream Detail, Task Create) + shared RuleEditor component + global rules API

**Architecture:** Add global `GET /api/v1/rules` endpoint on backend, extract shared RuleEditor from existing TaskForm, then build 3 pages sequentially.

**Tech Stack:** Rust/Axum (backend), React 19/Tailwind v4/TypeScript (frontend)

---

### Task 1: 后端 — 全局规则 API

**Files:**
- Modify: `src/api/rules.rs`
- Modify: `src/api/mod.rs`

- [ ] **Step 1: Add `list_all_rules` handler and types to `rules.rs`**

  Append to `src/api/rules.rs` before the helper functions:

  ```rust
  #[derive(Debug, Serialize, Deserialize, ToSchema)]
  pub struct GlobalRuleItem {
      #[schema(value_type = String)]
      pub stream_id: StreamId,
      pub stream_name: String,
      pub source_url: String,
      pub index: usize,
      pub rule: RuleConfig,
  }

  #[derive(Debug, Serialize, ToSchema)]
  pub struct GlobalRulesResponse {
      pub rules: Vec<GlobalRuleItem>,
  }

  #[derive(Debug, Deserialize)]
  pub struct GlobalRulesQuery {
      pub stream_id: Option<String>,
      pub r#type: Option<String>,
  }

  pub fn global_rules_routes(manager: StreamManager) -> Router {
      Router::new()
          .route("/", axum::routing::get(list_all_rules))
          .with_state(manager)
  }

  #[utoipa::path(
      get,
      path = "/api/v1/rules",
      tag = "rules",
      params(
          ("stream_id" = Option<String>, Query, description = "Filter by stream ID"),
          ("type" = Option<String>, Query, description = "Filter by rule type"),
      ),
      responses(
          (status = 200, description = "List of all rules across streams", body = GlobalRulesResponse),
      )
  )]
  pub async fn list_all_rules(
      State(manager): State<StreamManager>,
      Query(params): Query<GlobalRulesQuery>,
  ) -> Json<GlobalRulesResponse> {
      let registry = manager.registry();
      let streams = registry.list();
      let mut items = Vec::new();
      for info in &streams {
          let rules = info.rules.read().unwrap();
          for (i, rule) in rules.iter().enumerate() {
              if let Some(ref filter_type) = params.r#type {
                  let type_name = match rule {
                      RuleConfig::Interval { .. } => "interval",
                      RuleConfig::Fps { .. } => "fps",
                      RuleConfig::SceneChange { .. } => "scene_change",
                      RuleConfig::RateLimited { .. } => "rate_limited",
                      RuleConfig::Composite { .. } => "composite",
                  };
                  if type_name != filter_type.as_str() { continue; }
              }
              if let Some(ref sid_str) = params.stream_id {
                  if let Ok(sid) = uuid::Uuid::parse_str(sid_str) {
                      if info.id != sid {
                          continue;
                      }
                  }
              }
              items.push(GlobalRuleItem {
                  stream_id: info.id,
                  stream_name: info.config.name.clone(),
                  source_url: info.config.source_url.clone(),
                  index: i,
                  rule: rule.clone(),
              });
          }
      }
      Json(GlobalRulesResponse { rules: items })
  }
  ```

- [ ] **Step 2: Register global rules route in `api_router`**

  In `src/api/mod.rs`:

  ```rust
  pub fn api_router(manager: StreamManager, task_manager: Arc<TaskManager>, db_pool: Option<MySqlPool>) -> Router {
      let mut router = Router::new()
          .nest("/api/v1/streams", streams::stream_routes(manager.clone()))
          .nest("/api/v1/streams/{id}/rules", rules::rules_routes(manager.clone()))
          .nest("/api/v1/rules", rules::global_rules_routes(manager.clone()))
          .nest("/api/v1/tasks", tasks::task_routes(task_manager));
      // ...
  }
  ```

  Also add to `components(schemas(...))`:
  ```rust
  crate::api::rules::GlobalRuleItem,
  crate::api::rules::GlobalRulesResponse,
  ```

- [ ] **Step 3: Verify compilation**

  Run: `cargo check` on .122 build VM
  Expected: clean compilation (no errors)

- [ ] **Step 4: Commit**

  ```bash
  git add src/api/rules.rs src/api/mod.rs
  git commit -m "feat: add global rules API endpoint (GET /api/v1/rules)"
  ```

---

### Task 2: 前端 — RuleEditor 共享组件

**Files:**
- Create: `web/src/components/RuleEditor.tsx`
- Modify: `web/src/components/TaskForm.tsx`

- [ ] **Step 1: Create `RuleEditor.tsx`**

  Extract the rule editing logic from TaskForm into a reusable component:

  ```typescript
  import { useState } from "react"
  import type { RuleConfig } from "@/types/rule"

  const RULE_TYPES = ["interval", "fps", "scene_change", "rate_limited", "composite"] as const

  const RULE_LABELS: Record<string, string> = {
    interval: "定时抽帧",
    fps: "固定帧率",
    scene_change: "场景变化",
    rate_limited: "限速",
    composite: "复合规则",
  }

  const RULE_PARAM_LABELS: Record<string, string> = {
    interval: "间隔（秒）",
    fps: "FPS",
    scene_change: "阈值 (0.0~1.0)",
    rate_limited: "每分钟上限",
  }

  function getDefaultParam(type: string): string {
    switch (type) {
      case "interval": return "5"
      case "fps": return "10"
      case "scene_change": return "0.3"
      case "rate_limited": return "30"
      default: return ""
    }
  }

  function buildRule(type: string, param: string): RuleConfig {
    const rule: RuleConfig = { type: type as RuleConfig["type"] }
    switch (type) {
      case "interval": rule.interval_seconds = Number(param); break
      case "fps": rule.fps = Number(param); break
      case "scene_change": rule.threshold = Number(param); break
      case "rate_limited": rule.rule = { type: "interval", interval_seconds: 5 }; rule.max_per_minute = Number(param); break
    }
    return rule
  }

  function ruleSummary(rule: RuleConfig): string {
    switch (rule.type) {
      case "interval": return `每 ${rule.interval_seconds} 秒`
      case "fps": return `${rule.fps} FPS`
      case "scene_change": return `阈值 ${rule.threshold}`
      case "rate_limited": return `限速 ${rule.max_per_minute}/分钟`
      case "composite": return `复合 (${rule.operator})`
    }
  }

  export function RuleEditor({
    rules,
    onChange,
  }: {
    rules: RuleConfig[]
    onChange: (rules: RuleConfig[]) => void
  }) {
    const [editingType, setEditingType] = useState<string>("interval")
    const [editingParam, setEditingParam] = useState("5")

    const addRule = () => {
      if (!editingType) return
      onChange([...rules, buildRule(editingType, editingParam)])
    }

    const removeRule = (index: number) => {
      onChange(rules.filter((_, i) => i !== index))
    }

    return (
      <div className="space-y-3">
        <div>
          <label className="text-sm font-medium block mb-1">规则类型</label>
          <div className="flex gap-2 flex-wrap">
            {RULE_TYPES.map((t) => (
              <button key={t} type="button" onClick={() => { setEditingType(t); setEditingParam(getDefaultParam(t)) }}
                className={`px-3 py-1 text-sm border rounded-lg ${editingType === t ? "bg-brand text-white border-brand" : "hover:bg-gray-50"}`}
              >{RULE_LABELS[t]}</button>
            ))}
          </div>
        </div>
        {editingType !== "composite" && (
          <div>
            <label className="text-sm font-medium block mb-1">{RULE_PARAM_LABELS[editingType]}</label>
            <div className="flex gap-2">
              <input type="number" value={editingParam} onChange={(e) => setEditingParam(e.target.value)}
                className="border rounded-lg px-3 py-1.5 w-full text-sm" step="any" />
              <button type="button" onClick={addRule} className="px-4 py-1.5 text-sm bg-brand text-white rounded-lg hover:bg-blue-700 whitespace-nowrap">
                添加规则
              </button>
            </div>
          </div>
        )}
        {rules.length > 0 && (
          <div>
            <label className="text-sm font-medium block mb-1">已添加规则 ({rules.length})</label>
            <ul className="space-y-1">
              {rules.map((rule, i) => (
                <li key={i} className="flex items-center justify-between bg-gray-50 rounded-lg px-3 py-2 text-sm">
                  <span><span className="font-medium text-gray-500 mr-2">#{i + 1}</span>{RULE_LABELS[rule.type]} — {ruleSummary(rule)}</span>
                  <button type="button" onClick={() => removeRule(i)} className="text-error hover:text-red-800 text-xs font-medium">删除</button>
                </li>
              ))}
            </ul>
          </div>
        )}
      </div>
    )
  }
  ```

- [ ] **Step 2: Refactor `TaskForm.tsx` to use `RuleEditor`**

  Replace the rule editor section in `TaskForm.tsx`:

  ```typescript
  import { RuleEditor } from "./RuleEditor"

  // Inside component, replace rule-related state:
  const [rules, setRules] = useState<RuleConfig[]>([])

  // Replace the form submit logic:
  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    if (!streamId || rules.length === 0) return
    setSaving(true)
    try {
      await tasksApi.create({ name, stream_id: streamId, rules })
      onSave()
    } finally {
      setSaving(false)
    }
  }

  // Replace the rule type/param sections in JSX with:
  <RuleEditor rules={rules} onChange={setRules} />
  ```

- [ ] **Step 3: Verify TypeScript compilation**

  Run: `cd web && npx tsc --noEmit`
  Expected: no errors

- [ ] **Step 4: Commit**

  ```bash
  git add web/src/components/RuleEditor.tsx web/src/components/TaskForm.tsx
  git commit -m "refactor: extract RuleEditor component from TaskForm"
  ```

---

### Task 3: 前端 — 规则管理页

**Files:**
- Create: `web/src/api/rules.ts`
- Modify: `web/src/types/rule.ts`
- Create: `web/src/pages/RulesPage.tsx`
- Modify: `web/src/App.tsx`
- Modify: `web/src/components/Layout.tsx`

- [ ] **Step 1: Create `web/src/api/rules.ts`**

  ```typescript
  import { request } from "./client"
  import type { GlobalRuleItem } from "@/types/rule"

  export const rulesApi = {
    listGlobal: (params?: { stream_id?: string; type?: string }) => {
      const qs = new URLSearchParams()
      if (params?.stream_id) qs.set("stream_id", params.stream_id)
      if (params?.type) qs.set("type", params.type)
      const query = qs.toString()
      return request<{ rules: GlobalRuleItem[] }>(`/rules${query ? `?${query}` : ""}`)
    },
  }
  ```

- [ ] **Step 2: Add `GlobalRuleItem` type to `web/src/types/rule.ts`**

  ```typescript
  export interface GlobalRuleItem {
    stream_id: string
    stream_name: string
    source_url: string
    index: number
    rule: RuleConfig
  }
  ```

- [ ] **Step 3: Create `web/src/pages/RulesPage.tsx`**

  ```typescript
  import { useState, useEffect, useCallback } from "react"
  import { useNavigate } from "react-router-dom"
  import { rulesApi } from "@/api/rules"
  import { streamsApi } from "@/api/streams"
  import type { GlobalRuleItem, RuleConfig } from "@/types/rule"
  import type { StreamInfo } from "@/types/stream"
  import { RuleEditor } from "@/components/RuleEditor"

  const RULE_LABELS: Record<string, string> = {
    interval: "定时抽帧",
    fps: "固定帧率",
    scene_change: "场景变化",
    rate_limited: "限速",
    composite: "复合规则",
  }

  function ruleSummary(rule: RuleConfig): string {
    switch (rule.type) {
      case "interval": return `每 ${rule.interval_seconds} 秒`
      case "fps": return `${rule.fps} FPS`
      case "scene_change": return `阈值 ${rule.threshold}`
      case "rate_limited": return `限速 ${rule.max_per_minute}/分钟`
      case "composite": return `复合 (${rule.operator})`
    }
  }

  export default function RulesPage() {
    const [items, setItems] = useState<GlobalRuleItem[]>([])
    const [streams, setStreams] = useState<StreamInfo[]>([])
    const [filterStreamId, setFilterStreamId] = useState("")
    const [filterType, setFilterType] = useState("")
    const [loading, setLoading] = useState(true)
    const navigate = useNavigate()

    const load = useCallback(() => {
      setLoading(true)
      const params: { stream_id?: string; type?: string } = {}
      if (filterStreamId) params.stream_id = filterStreamId
      if (filterType) params.type = filterType
      rulesApi.listGlobal(params).then((res) => setItems(res.rules)).catch(() => {}).finally(() => setLoading(false))
    }, [filterStreamId, filterType])

    useEffect(() => { load() }, [load])
    useEffect(() => { streamsApi.list().then((res) => setStreams(res.streams)).catch(() => {}) }, [])

    return (
      <div className="space-y-4">
        <div className="flex items-center justify-between">
          <h1 className="text-xl font-bold">规则管理</h1>
        </div>

        <div className="flex gap-3 items-center">
          <select value={filterStreamId} onChange={(e) => setFilterStreamId(e.target.value)} className="border rounded-lg px-3 py-1.5 text-sm">
            <option value="">全部流</option>
            {streams.map((s) => <option key={s.id} value={s.id}>{s.name}</option>)}
          </select>
          <select value={filterType} onChange={(e) => setFilterType(e.target.value)} className="border rounded-lg px-3 py-1.5 text-sm">
            <option value="">全部类型</option>
            {Object.entries(RULE_LABELS).map(([k, v]) => <option key={k} value={k}>{v}</option>)}
          </select>
          <button onClick={load} className="px-3 py-1.5 text-sm border rounded-lg hover:bg-gray-50">刷新</button>
        </div>

        <div className="bg-white border rounded-xl shadow-sm overflow-hidden">
          {loading ? (
            <div className="p-8 text-center text-gray-400">加载中...</div>
          ) : items.length === 0 ? (
            <div className="p-8 text-center text-gray-400">暂无规则</div>
          ) : (
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b bg-gray-50 text-left">
                  <th className="px-4 py-3 font-medium text-gray-600">流名称</th>
                  <th className="px-4 py-3 font-medium text-gray-600">规则类型</th>
                  <th className="px-4 py-3 font-medium text-gray-600">配置</th>
                  <th className="px-4 py-3 font-medium text-gray-600">索引</th>
                </tr>
              </thead>
              <tbody>
                {items.map((item, i) => (
                  <tr key={`${item.stream_id}-${item.index}`} className="border-b last:border-0 hover:bg-gray-50">
                    <td className="px-4 py-3">
                      <button onClick={() => navigate(`/streams/${item.stream_id}`)} className="text-brand hover:underline font-medium text-left">{item.stream_name}</button>
                      <div className="text-xs text-gray-400 mt-0.5 truncate max-w-xs">{item.source_url}</div>
                    </td>
                    <td className="px-4 py-3">
                      <span className="px-1.5 py-0.5 rounded text-xs font-medium bg-blue-50 text-blue-700">
                        {RULE_LABELS[item.rule.type] || item.rule.type}
                      </span>
                    </td>
                    <td className="px-4 py-3 text-gray-600">{ruleSummary(item.rule)}</td>
                    <td className="px-4 py-3 text-gray-400 text-xs">{item.index}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </div>
      </div>
    )
  }
  ```

- [ ] **Step 4: Add route + nav link**

  In `web/src/App.tsx`:
  ```typescript
  import RulesPage from "@/pages/RulesPage"
  // Add route:
  <Route path="rules" element={<RulesPage />} />
  ```

  In `web/src/components/Layout.tsx`:
  ```typescript
  const navItems = [
    { to: "/", label: "控制面板" },
    { to: "/streams", label: "流管理" },
    { to: "/rules", label: "规则管理" },
    { to: "/tasks", label: "任务管理" },
    { to: "/activity", label: "活动日志" },
  ]
  ```

- [ ] **Step 5: Verify TypeScript compilation**

  Run: `cd web && npx tsc --noEmit`
  Expected: no errors

- [ ] **Step 6: Commit**

  ```bash
  git add web/src/api/rules.ts web/src/types/rule.ts web/src/pages/RulesPage.tsx web/src/App.tsx web/src/components/Layout.tsx
  git commit -m "feat: add rules management page (global view)"
  ```

---

### Task 4: 前端 — 流详情页

**Files:**
- Create: `web/src/pages/StreamDetail.tsx`
- Modify: `web/src/App.tsx`
- Modify (optional): `web/src/components/StreamTable.tsx` if needed

- [ ] **Step 1: Create `web/src/pages/StreamDetail.tsx`**

  ```typescript
  import { useState, useEffect, useCallback } from "react"
  import { useParams, useNavigate } from "react-router-dom"
  import { streamsApi } from "@/api/streams"
  import { StreamForm } from "@/components/StreamForm"
  import { FramePreview } from "@/components/FramePreview"
  import type { StreamInfo } from "@/types/stream"

  const STATUS_LABELS: Record<string, string> = { online: "在线", offline: "离线", error: "错误", connecting: "连接中" }
  const STATUS_STYLES: Record<string, string> = {
    online: "bg-green-50 text-green-700",
    offline: "bg-gray-50 text-gray-600",
    error: "bg-red-50 text-red-700",
    connecting: "bg-yellow-50 text-yellow-700",
  }

  export default function StreamDetail() {
    const { id } = useParams<{ id: string }>()
    const navigate = useNavigate()
    const [stream, setStream] = useState<StreamInfo | null>(null)
    const [showEdit, setShowEdit] = useState(false)
    const [loading, setLoading] = useState(true)

    const load = useCallback(() => {
      if (!id) return
      setLoading(true)
      streamsApi.get(id).then(setStream).catch(() => navigate("/streams")).finally(() => setLoading(false))
    }, [id, navigate])

    useEffect(() => { load() }, [load])

    const handleDelete = async () => {
      if (!id || !confirm("确定删除该流？")) return
      await streamsApi.delete(id)
      navigate("/streams")
    }

    if (loading) return <div className="p-8 text-center text-gray-400">加载中...</div>
    if (!stream) return <div className="p-8 text-center text-gray-400">流不存在</div>

    return (
      <div className="space-y-6">
        <button onClick={() => navigate("/streams")} className="text-sm text-brand hover:underline">&larr; 返回流列表</button>

        {/* Info Card */}
        <div className="bg-white border rounded-xl p-5 shadow-sm">
          <div className="flex justify-between items-start mb-4">
            <div>
              <h1 className="text-xl font-bold">{stream.name}</h1>
              <div className="text-xs text-gray-400 mt-1 font-mono">{stream.id}</div>
            </div>
            <span className={`px-2 py-1 rounded text-xs font-medium ${STATUS_STYLES[stream.status] || ""}`}>
              {STATUS_LABELS[stream.status] || stream.status}
            </span>
          </div>
          <div className="grid grid-cols-2 gap-4 text-sm">
            <div>
              <span className="text-gray-500">源 URL</span>
              <div className="font-mono text-xs mt-0.5 truncate">{stream.source_url}</div>
            </div>
            <div>
              <span className="text-gray-500">类型</span>
              <div>{(stream.source_type || "").toUpperCase()}</div>
            </div>
            <div>
              <span className="text-gray-500">解码帧数</span>
              <div>{stream.frames_decoded}</div>
            </div>
            <div>
              <span className="text-gray-500">抽帧数</span>
              <div>{stream.frames_extracted}</div>
            </div>
            <div>
              <span className="text-gray-500">当前 FPS</span>
              <div>{stream.frames_per_hour ? (stream.frames_per_hour / 3600).toFixed(2) : "—"}</div>
            </div>
            <div>
              <span className="text-gray-500">在线时长</span>
              <div>{stream.uptime_seconds ? `${Math.floor(stream.uptime_seconds / 60)} 分钟` : "—"}</div>
            </div>
          </div>
          <div className="flex gap-2 mt-4 pt-4 border-t">
            <button onClick={() => setShowEdit(true)} className="px-3 py-1.5 text-sm border rounded-lg hover:bg-gray-50">编辑</button>
            <button onClick={handleDelete} className="px-3 py-1.5 text-sm border rounded-lg text-error hover:bg-red-50">删除</button>
          </div>
        </div>

        {/* Latest Frame */}
        <div className="bg-white border rounded-xl p-5 shadow-sm">
          <h2 className="text-base font-semibold mb-3">最新帧</h2>
          <FramePreview streamId={id!} />
        </div>

        {/* Edit Modal */}
        {showEdit && <StreamForm stream={stream} onClose={() => setShowEdit(false)} onSave={() => { setShowEdit(false); load() }} />}
      </div>
    )
  }
  ```

  Note: The existing `StreamForm` component may not support an edit mode with initial values. Check if it does: it accepts `stream?: StreamInfo` as a prop for editing. If it doesn't, we'll need a quick inline form instead — but looking at the existing component, it does pass `stream` as `initialConfig`.

- [ ] **Step 2: Add route**

  In `web/src/App.tsx`:
  ```typescript
  import StreamDetail from "@/pages/StreamDetail"
  // Add route:
  <Route path="streams/:id" element={<StreamDetail />} />
  ```

  **Important:** Place this BEFORE the `streams` index route to avoid matching conflicts. Actually, in react-router v7, `/streams` and `/streams/:id` can coexist — the index route matches `/streams` exactly, and `/streams/:id` matches the detail route.

- [ ] **Step 3: Make stream name clickable in `StreamTable.tsx`**

  In `web/src/components/StreamTable.tsx`, find the stream name cell and wrap it in a Link/button:

  ```typescript
  // Import:
  import { useNavigate } from "react-router-dom"
  // In component:
  const navigate = useNavigate()
  // In the stream name cell:
  <button onClick={() => navigate(`/streams/${s.id}`)} className="text-brand hover:underline font-medium text-left">
    {s.name}
  </button>
  ```

- [ ] **Step 4: Verify TypeScript compilation**

  Run: `cd web && npx tsc --noEmit`
  Expected: no errors

- [ ] **Step 5: Commit**

  ```bash
  git add web/src/pages/StreamDetail.tsx web/src/App.tsx web/src/components/StreamTable.tsx
  git commit -m "feat: add stream detail page"
  ```

---

### Task 5: 前端 — 任务创建页

**Files:**
- Create: `web/src/pages/TaskCreatePage.tsx`
- Modify: `web/src/App.tsx`
- Modify: `web/src/components/Layout.tsx` (already done in Task 3, skip if committed)

- [ ] **Step 1: Create `web/src/pages/TaskCreatePage.tsx`**

  ```typescript
  import { useState, useEffect } from "react"
  import { useNavigate } from "react-router-dom"
  import { tasksApi } from "@/api/tasks"
  import { streamsApi } from "@/api/streams"
  import { RuleEditor } from "@/components/RuleEditor"
  import type { StreamInfo } from "@/types/stream"
  import type { RuleConfig } from "@/types/rule"

  export default function TaskCreatePage() {
    const navigate = useNavigate()
    const [name, setName] = useState("")
    const [streamId, setStreamId] = useState("")
    const [rules, setRules] = useState<RuleConfig[]>([])
    const [streams, setStreams] = useState<StreamInfo[]>([])
    const [saving, setSaving] = useState(false)

    useEffect(() => {
      streamsApi.list().then((res) => setStreams(res.streams)).catch(() => {})
    }, [])

    const handleSubmit = async (e: React.FormEvent) => {
      e.preventDefault()
      if (!streamId || rules.length === 0) return
      setSaving(true)
      try {
        const task = await tasksApi.create({ name, stream_id: streamId, rules })
        navigate(`/tasks/${task.id}`)
      } catch {
        setSaving(false)
      }
    }

    return (
      <div className="max-w-2xl mx-auto">
        <button onClick={() => navigate("/tasks")} className="text-sm text-brand hover:underline mb-4 inline-block">&larr; 返回任务列表</button>
        <h1 className="text-xl font-bold mb-6">新建任务</h1>

        <form onSubmit={handleSubmit} className="space-y-6">
          <div className="bg-white border rounded-xl p-5 shadow-sm space-y-4">
            <div>
              <label className="text-sm font-medium block mb-1">任务名称</label>
              <input required value={name} onChange={(e) => setName(e.target.value)}
                className="border rounded-lg px-3 py-1.5 w-full text-sm" placeholder="输入任务名称" />
            </div>
            <div>
              <label className="text-sm font-medium block mb-1">关联流</label>
              <select required value={streamId} onChange={(e) => setStreamId(e.target.value)}
                className="border rounded-lg px-3 py-1.5 w-full text-sm">
                <option value="">选择流...</option>
                {streams.map((s) => <option key={s.id} value={s.id}>{s.name}</option>)}
              </select>
            </div>
          </div>

          <div className="bg-white border rounded-xl p-5 shadow-sm">
            <h2 className="text-base font-semibold mb-3">抽帧规则</h2>
            <RuleEditor rules={rules} onChange={setRules} />
          </div>

          <div className="flex justify-end gap-3">
            <button type="button" onClick={() => navigate("/tasks")}
              className="px-4 py-1.5 text-sm border rounded-lg hover:bg-gray-50">取消</button>
            <button type="submit" disabled={saving || rules.length === 0}
              className="px-4 py-1.5 text-sm bg-brand text-white rounded-lg hover:bg-blue-700 disabled:opacity-50">
              {saving ? "创建中..." : "创建任务"}
            </button>
          </div>
        </form>
      </div>
    )
  }
  ```

- [ ] **Step 2: Add route**

  In `web/src/App.tsx`:
  ```typescript
  import TaskCreatePage from "@/pages/TaskCreatePage"
  // Add route (BEFORE the /tasks/:id route):
  <Route path="tasks/create" element={<TaskCreatePage />} />
  <Route path="tasks/:id" element={<TaskDetail />} />
  ```

  Note: `/tasks/create` must come before `/tasks/:id` in react-router.

- [ ] **Step 3: Verify TypeScript compilation**

  Run: `cd web && npx tsc --noEmit`
  Expected: no errors

- [ ] **Step 4: Commit**

  ```bash
  git add web/src/pages/TaskCreatePage.tsx web/src/App.tsx
  git commit -m "feat: add task creation page"
  ```

---

### Task 6: 构建、部署与 E2E 验证

- [ ] **Step 1: Sync all code to .122 and build**

  ```bash
  scp Cargo.toml Cargo.lock taplo@192.168.3.122:/home/taplo/getframe/
  scp -r src/ taplo@192.168.3.122:/home/taplo/getframe/
  scp -r migrations/ taplo@192.168.3.122:/home/taplo/getframe/
  ssh taplo@192.168.3.122 "cd /home/taplo/getframe && docker exec getframe-compile cargo build --release --bin getframe-worker 2>&1 | tail -5"
  ```

- [ ] **Step 2: Deploy binary to .123**

  Copy binary via HTTP relay (start server on .122, `curl` on .123), then build Docker image:

  ```bash
  # On .122: screen -dmS httpserv python3 -m http.server 18999 --bind 0.0.0.0
  # On .123: curl -s -o getframe-worker-new http://192.168.3.122:18999/getframe-worker-new
  # Build: docker build --network host -t getframe-worker-tmp:latest -f /tmp/Dockerfile.quick /home/taplo/getframe
  ```

- [ ] **Step 3: Restart worker and run E2E test**

  ```bash
  # Stop old worker, start with new image
  WORKER_IMAGE=getframe-worker-tmp:latest docker compose up -d worker
  # Wait for health: curl -sf http://localhost:8080/health
  # Run E2E: python3 tests/e2e/test_full_flow.py
  ```

- [ ] **Step 4: Clean up HTTP server on .122**

  ```bash
  ssh taplo@192.168.3.122 "screen -S httpserv -X quit"
  ```

- [ ] **Step 5: Final commit (if any late fixes)**

  ```bash
  git add -A
  git commit -m "fix: address review feedback"
  ```
