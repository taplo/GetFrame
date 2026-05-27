import { BrowserRouter, Routes, Route } from "react-router-dom"
import { Layout } from "@/components/Layout"
import { Dashboard } from "@/pages/Dashboard"
import { StreamsPage } from "@/pages/Streams"
import { TasksPage } from "@/pages/Tasks"
import { TaskDetail } from "@/pages/TaskDetail"

export function App() {
  return (
    <BrowserRouter>
      <Routes>
        <Route element={<Layout />}>
          <Route index element={<Dashboard />} />
          <Route path="streams" element={<StreamsPage />} />
          <Route path="tasks" element={<TasksPage />} />
          <Route path="tasks/:id" element={<TaskDetail />} />
        </Route>
      </Routes>
    </BrowserRouter>
  )
}
