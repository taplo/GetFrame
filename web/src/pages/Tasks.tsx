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
