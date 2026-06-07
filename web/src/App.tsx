import { BrowserRouter, Routes, Route } from "react-router-dom"
import { Layout } from "@/components/Layout"
import { Dashboard } from "@/pages/Dashboard"
import { StreamsPage } from "@/pages/Streams"
import { TasksPage } from "@/pages/Tasks"
import { TaskDetail } from "@/pages/TaskDetail"
import TaskCreatePage from "@/pages/TaskCreatePage"
import ActivityLog from "@/pages/ActivityLog"
import RulesPage from "@/pages/RulesPage"
import StreamDetail from "@/pages/StreamDetail"

export function App() {
  return (
    <BrowserRouter>
      <Routes>
        <Route element={<Layout />}>
          <Route index element={<Dashboard />} />
          <Route path="streams" element={<StreamsPage />} />
          <Route path="streams/:id" element={<StreamDetail />} />
          <Route path="tasks" element={<TasksPage />} />
          <Route path="tasks/create" element={<TaskCreatePage />} />
          <Route path="tasks/:id" element={<TaskDetail />} />
          <Route path="activity" element={<ActivityLog />} />
          <Route path="rules" element={<RulesPage />} />
        </Route>
      </Routes>
    </BrowserRouter>
  )
}
