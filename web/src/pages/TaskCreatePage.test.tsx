import { describe, it, expect, vi, beforeEach } from "vitest"
import { render, screen, waitFor, fireEvent } from "@testing-library/react"
import { MemoryRouter, Route, Routes } from "react-router-dom"
import TaskCreatePage from "./TaskCreatePage"

const mockStreamsList = vi.fn()
const mockTaskCreate = vi.fn()

vi.mock("@/api/tasks", () => ({
  tasksApi: {
    create: (data: { name: string; stream_id: string; rules: unknown[] }) => mockTaskCreate(data),
  },
}))

vi.mock("@/api/streams", () => ({
  streamsApi: {
    list: () => mockStreamsList(),
  },
}))

function renderPage() {
  return render(
    <MemoryRouter initialEntries={["/tasks/create"]}>
      <Routes>
        <Route path="/tasks/create" element={<TaskCreatePage />} />
        <Route path="/tasks/:id" element={<div data-testid="task-detail">Task Detail</div>} />
        <Route path="/tasks" element={<div data-testid="tasks-list">Tasks List</div>} />
      </Routes>
    </MemoryRouter>
  )
}

describe("TaskCreatePage", () => {
  beforeEach(() => {
    vi.clearAllMocks()
    mockStreamsList.mockResolvedValue({ streams: [] })
  })

  it("renders the form title", async () => {
    mockTaskCreate.mockImplementation(() => new Promise(() => {}))
    renderPage()
    await waitFor(() => {
      expect(screen.getByText("新建任务")).toBeTruthy()
    })
  })

  it("renders task name input", async () => {
    renderPage()
    await waitFor(() => {
      expect(screen.getByPlaceholderText("输入任务名称")).toBeTruthy()
    })
  })

  it("renders stream selector", async () => {
    mockStreamsList.mockResolvedValue({
      streams: [
        { id: "s1", name: "camera-1" },
        { id: "s2", name: "camera-2" },
      ],
    })
    renderPage()
    await waitFor(() => {
      expect(screen.getByText("camera-1")).toBeTruthy()
      expect(screen.getByText("camera-2")).toBeTruthy()
    })
  })

  it("renders RuleEditor section", async () => {
    renderPage()
    await waitFor(() => {
      expect(screen.getByText("抽帧规则")).toBeTruthy()
    })
    expect(screen.getByText("定时抽帧")).toBeTruthy()
  })

  it("renders cancel and submit buttons", async () => {
    renderPage()
    await waitFor(() => {
      expect(screen.getByText("取消")).toBeTruthy()
      expect(screen.getByText("创建任务")).toBeTruthy()
    })
  })

  it("submit button is disabled when no rules", async () => {
    renderPage()
    await waitFor(() => {
      const btn = screen.getByText("创建任务") as HTMLButtonElement
      expect(btn.disabled).toBe(true)
    })
  })

  it("submit button enables after adding a rule", async () => {
    mockStreamsList.mockResolvedValue({
      streams: [{ id: "s1", name: "camera-1" }],
    })
    renderPage()
    await waitFor(() => {
      expect(screen.getByText("camera-1")).toBeTruthy()
    })
    fireEvent.change(screen.getByPlaceholderText("输入任务名称"), { target: { value: "my-task" } })
    const selectEl = screen.getByRole("combobox") as HTMLSelectElement
    fireEvent.change(selectEl, { target: { value: "s1" } })
    fireEvent.click(screen.getByText("添加规则"))
    const btn = screen.getByText("创建任务") as HTMLButtonElement
    expect(btn.disabled).toBe(false)
  })

  it("calls create API on form submit", async () => {
    mockStreamsList.mockResolvedValue({
      streams: [{ id: "s1", name: "camera-1" }],
    })
    mockTaskCreate.mockResolvedValue({ id: "task-1" })
    renderPage()
    await waitFor(() => {
      expect(screen.getByText("camera-1")).toBeTruthy()
    })
    fireEvent.change(screen.getByPlaceholderText("输入任务名称"), { target: { value: "my-task" } })
    const selectEl = screen.getByRole("combobox") as HTMLSelectElement
    fireEvent.change(selectEl, { target: { value: "s1" } })
    fireEvent.click(screen.getByText("添加规则"))
    fireEvent.click(screen.getByText("创建任务"))
    await waitFor(() => {
      expect(mockTaskCreate).toHaveBeenCalledWith({
        name: "my-task",
        stream_id: "s1",
        rules: [{ type: "interval", interval_seconds: 5 }],
      })
    })
  })

  it("navigates to task detail after creation", async () => {
    mockStreamsList.mockResolvedValue({
      streams: [{ id: "s1", name: "camera-1" }],
    })
    mockTaskCreate.mockResolvedValue({ id: "task-1" })
    renderPage()
    await waitFor(() => {
      expect(screen.getByText("camera-1")).toBeTruthy()
    })
    fireEvent.change(screen.getByPlaceholderText("输入任务名称"), { target: { value: "my-task" } })
    const selectEl = screen.getByRole("combobox") as HTMLSelectElement
    fireEvent.change(selectEl, { target: { value: "s1" } })
    fireEvent.click(screen.getByText("添加规则"))
    fireEvent.click(screen.getByText("创建任务"))
    await waitFor(() => {
      expect(screen.getByTestId("task-detail")).toBeTruthy()
    })
  })

  it("navigates to tasks list on cancel", async () => {
    renderPage()
    await waitFor(() => {
      expect(screen.getByText("取消")).toBeTruthy()
    })
    fireEvent.click(screen.getByText("取消"))
    await waitFor(() => {
      expect(screen.getByTestId("tasks-list")).toBeTruthy()
    })
  })
})
