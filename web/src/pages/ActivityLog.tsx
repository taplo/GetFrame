import { useState, useEffect, useCallback } from "react";
import { activityApi } from "../api/activity";
import type { ActivityQuery, ActivityListResponse } from "../types/activity";

const EVENT_TYPE_OPTIONS = [
  { value: "", label: "全部类型" },
  { value: "stream.", label: "流操作" },
  { value: "task.", label: "任务操作" },
  { value: "auth.", label: "认证操作" },
  { value: "worker.", label: "Worker 操作" },
];

const RESOURCE_TYPE_OPTIONS = [
  { value: "", label: "全部资源" },
  { value: "stream", label: "流" },
  { value: "task", label: "任务" },
  { value: "user", label: "用户" },
  { value: "api_key", label: "API Key" },
  { value: "system", label: "系统" },
];

function formatTime(iso: string): string {
  const d = new Date(iso);
  return d.toLocaleTimeString("zh-CN", { hour: "2-digit", minute: "2-digit", second: "2-digit" });
}

function formatDate(iso: string): string {
  const d = new Date(iso);
  return d.toLocaleDateString("zh-CN", { month: "2-digit", day: "2-digit" });
}

function resourceBadge(type: string): { label: string; color: string } {
  const map: Record<string, { label: string; color: string }> = {
    stream: { label: "流", color: "bg-blue-100 text-blue-800" },
    task: { label: "任务", color: "bg-green-100 text-green-800" },
    user: { label: "用户", color: "bg-purple-100 text-purple-800" },
    api_key: { label: "API Key", color: "bg-orange-100 text-orange-800" },
    system: { label: "系统", color: "bg-gray-100 text-gray-800" },
  };
  return map[type] || { label: type, color: "bg-gray-100 text-gray-800" };
}

export default function ActivityLog() {
  const [items, setItems] = useState<ActivityListResponse["items"]>([]);
  const [total, setTotal] = useState(0);
  const [page, setPage] = useState(1);
  const [pageSize] = useState(50);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const [eventTypeFilter, setEventTypeFilter] = useState("");
  const [resourceTypeFilter, setResourceTypeFilter] = useState("");
  const [searchText, setSearchText] = useState("");
  const [debouncedSearch, setDebouncedSearch] = useState("");

  useEffect(() => {
    const timer = setTimeout(() => setDebouncedSearch(searchText), 300);
    return () => clearTimeout(timer);
  }, [searchText]);

  const fetchData = useCallback(async (p: number) => {
    setLoading(true);
    setError(null);
    try {
      const query: ActivityQuery = {
        page: p,
        page_size: pageSize,
        search: debouncedSearch || undefined,
      };
      if (eventTypeFilter) query.event_type = eventTypeFilter;
      if (resourceTypeFilter) query.resource_type = resourceTypeFilter;

      const data = await activityApi.list(query);
      setItems(data.items);
      setTotal(data.total);
      setPage(data.page);
    } catch {
      setError("加载活动日志失败");
      setItems([]);
    } finally {
      setLoading(false);
    }
  }, [eventTypeFilter, resourceTypeFilter, debouncedSearch, pageSize]);

  useEffect(() => {
    fetchData(1);
  }, [fetchData]);

  const totalPages = Math.ceil(total / pageSize);

  const handleExport = async () => {
    try {
      await activityApi.exportCsv({
        event_type: eventTypeFilter || undefined,
        resource_type: resourceTypeFilter || undefined,
        search: debouncedSearch || undefined,
      });
    } catch {
      // silent
    }
  };

  return (
    <div className="space-y-4">
      <h1 className="text-2xl font-bold">活动日志</h1>

      <div className="flex flex-wrap gap-3 items-end">
        <select
          value={eventTypeFilter}
          onChange={(e) => { setEventTypeFilter(e.target.value); setPage(1); }}
          className="border rounded px-3 py-2 text-sm"
        >
          {EVENT_TYPE_OPTIONS.map((o) => (
            <option key={o.value} value={o.value}>{o.label}</option>
          ))}
        </select>

        <select
          value={resourceTypeFilter}
          onChange={(e) => { setResourceTypeFilter(e.target.value); setPage(1); }}
          className="border rounded px-3 py-2 text-sm"
        >
          {RESOURCE_TYPE_OPTIONS.map((o) => (
            <option key={o.value} value={o.value}>{o.label}</option>
          ))}
        </select>

        <input
          type="text"
          placeholder="搜索描述..."
          value={searchText}
          onChange={(e) => setSearchText(e.target.value)}
          className="border rounded px-3 py-2 text-sm flex-1 min-w-[200px]"
        />

        <button
          onClick={handleExport}
          className="bg-brand text-white rounded px-4 py-2 text-sm hover:opacity-90"
        >
          导出 CSV
        </button>
      </div>

      {error && (
        <div className="bg-red-50 text-red-700 rounded p-3 text-sm">
          {error}
          <button onClick={() => fetchData(page)} className="ml-3 underline">重试</button>
        </div>
      )}

      {loading && (
        <div className="space-y-2">
          {Array.from({ length: 5 }).map((_, i) => (
            <div key={i} className="h-12 bg-gray-100 rounded animate-pulse" />
          ))}
        </div>
      )}

      {!loading && !error && items.length === 0 && (
        <div className="text-center py-12 text-gray-500">暂无活动记录</div>
      )}

      {!loading && items.length > 0 && (
        <div className="bg-white rounded shadow-sm">
          {items.map((item) => {
            const badge = resourceBadge(item.resource_type);
            return (
              <div key={item.id} className="flex items-center gap-3 px-4 py-3 border-b last:border-0 hover:bg-gray-50">
                <span className="text-gray-400 text-xs w-12 shrink-0" title={item.recorded_at}>
                  {formatDate(item.recorded_at)} {formatTime(item.recorded_at)}
                </span>
                <span className={`text-xs font-medium px-2 py-0.5 rounded ${badge.color}`}>{badge.label}</span>
                <span className="flex-1 text-sm">{item.description}</span>
                <span className="text-xs text-gray-400 shrink-0">{item.actor}</span>
              </div>
            );
          })}
        </div>
      )}

      {totalPages > 1 && (
        <div className="flex justify-center items-center gap-4 text-sm">
          <button
            disabled={page <= 1}
            onClick={() => fetchData(page - 1)}
            className="px-3 py-1 border rounded disabled:opacity-50 hover:bg-gray-50"
          >
            ← 上一页
          </button>
          <span className="text-gray-500">第 {page}/{totalPages} 页</span>
          <button
            disabled={page >= totalPages}
            onClick={() => fetchData(page + 1)}
            className="px-3 py-1 border rounded disabled:opacity-50 hover:bg-gray-50"
          >
            下一页 →
          </button>
        </div>
      )}
    </div>
  );
}
