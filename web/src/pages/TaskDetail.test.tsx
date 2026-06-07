import { describe, it, expect, vi, beforeEach } from "vitest"
import { render, screen, waitFor, fireEvent } from "@testing-library/react"
import { MemoryRouter, Route, Routes } from "react-router-dom"
import { TaskDetail } from "./TaskDetail"

const mockGet = vi.fn()
const mockStreamsGet = vi.fn()
const mockEvents = vi.fn()
const mockStart = vi.fn()
const mockPause = vi.fn()
const mockResume = vi.fn()
const mockStop = vi.fn()
const mockDelete = vi.fn()

vi.mock("@/api/tasks", () => ({
  tasksApi: {
    get: (id: string) => mockGet(id),
    events: (id: string) => mockEvents(id),
    start: (id: string) => mockStart(id),
    pause: (id: string) => mockPause(id),
    resume: (id: string) => mockResume(id),
    stop: (id: string) => mockStop(id),
    delete: (id: string) => mockDelete(id),
  },
}))

vi.mock("@/api/streams", () => ({
  streamsApi: {
    get: (id: string) => mockStreamsGet(id),
  },
}))

vi.mock("@/components/FramePreview", () => ({
  FramePreview: () => <div data-testid="frame-preview">Frame</div>,
}))

function renderPage(taskId: string = "task-1") {
  return render(
    <MemoryRouter initialEntries={[`/tasks/${taskId}`]}>
      <Routes>
        <Route path="/tasks/:id" element={<TaskDetail />} />
        <Route path="/streams/:id" element={<div data-testid="stream-detail">Stream Detail</div>} />
      </Routes>
    </MemoryRouter>
  )
}

function makeTask(overrides: Record<string, unknown> = {}) {
  return {
    id: "task-1",
    name: "test-task",
    stream_id: "stream-1",
    stream_name: "camera-1",
    status: "Created",
    rules: [{ type: "interval", interval_seconds: 5 }],
    frames_extracted: 0,
    created_at: "2026-06-01T00:00:00Z",
    ...overrides,
  }
}

describe("TaskDetail", () => {
  beforeEach(() => {
    vi.clearAllMocks()
    mockEvents.mockResolvedValue({ events: [] })
    mockStreamsGet.mockResolvedValue({ id: "stream-1", name: "camera-1" })
  })

  it("shows loading state initially", () => {
    mockGet.mockImplementation(() => new Promise(() => {}))
    renderPage()
    expect(screen.getByText("加载中...")).toBeTruthy()
  })

  it("renders task info when loaded", async () => {
    mockGet.mockResolvedValue(makeTask())
    renderPage()
    await waitFor(() => {
      expect(screen.getByText("test-task")).toBeTruthy()
    })
    expect(screen.getByText("已创建")).toBeTruthy()
    expect(screen.getByText("0")).toBeTruthy()
    expect(screen.getByText("camera-1")).toBeTruthy()
  })

  it("renders rules list", async () => {
    mockGet.mockResolvedValue(makeTask({
      rules: [
        { type: "interval", interval_seconds: 5 },
        { type: "fps", fps: 10 },
      ],
    }))
    renderPage()
    await waitFor(() => {
      expect(screen.getByText(/— 每 5 秒$/)).toBeTruthy()
      expect(screen.getByText(/— 10 FPS$/)).toBeTruthy()
    })
  })

  it("shows error state on API failure", async () => {
    mockGet.mockRejectedValue(new Error("network error"))
    renderPage()
    await waitFor(() => {
      expect(screen.getByText("加载失败")).toBeTruthy()
    })
    expect(screen.getByText("重试")).toBeTruthy()
  })

  it("shows not found state on 404", async () => {
    const err = new Error("not found")
    ;(err as any).status = 404
    mockGet.mockRejectedValue(err)
    renderPage()
    await waitFor(() => {
      expect(screen.getByText("任务不存在")).toBeTruthy()
    })
  })

  it("shows start button for Created status", async () => {
    mockGet.mockResolvedValue(makeTask({ status: "Created" }))
    renderPage()
    await waitFor(() => {
      expect(screen.getByText("启动")).toBeTruthy()
    })
  })

  it("shows pause button for Running status", async () => {
    mockGet.mockResolvedValue(makeTask({ status: "Running", started_at: "2026-06-01T01:00:00Z" }))
    renderPage()
    await waitFor(() => {
      expect(screen.getByText("暂停")).toBeTruthy()
    })
  })

  it("shows resume button for Paused status", async () => {
    mockGet.mockResolvedValue(makeTask({ status: "Paused", started_at: "2026-06-01T01:00:00Z" }))
    renderPage()
    await waitFor(() => {
      expect(screen.getByText("恢复")).toBeTruthy()
    })
  })

  it("shows stop button for Running and Paused", async () => {
    mockGet.mockResolvedValue(makeTask({ status: "Running", started_at: "2026-06-01T01:00:00Z" }))
    renderPage()
    await waitFor(() => {
      expect(screen.getByText("停止")).toBeTruthy()
    })
  })

  it("shows delete button for non-running states", async () => {
    mockGet.mockResolvedValue(makeTask({ status: "Stopped" }))
    renderPage()
    await waitFor(() => {
      expect(screen.getByText("删除")).toBeTruthy()
    })
  })

  it("calls start action on button click", async () => {
    mockGet.mockResolvedValue(makeTask({ status: "Created" }))
    mockStart.mockResolvedValue(makeTask({ status: "Running" }))
    renderPage()
    await waitFor(() => {
      expect(screen.getByText("启动")).toBeTruthy()
    })
    fireEvent.click(screen.getByText("启动"))
    await waitFor(() => {
      expect(mockStart).toHaveBeenCalledWith("task-1")
    })
  })

  it("shows frame preview after load", async () => {
    mockGet.mockResolvedValue(makeTask())
    renderPage()
    await waitFor(() => {
      expect(screen.getByTestId("frame-preview")).toBeTruthy()
    })
  })

  it("shows event timeline after load", async () => {
    mockGet.mockResolvedValue(makeTask({ status: "Stopped", frames_extracted: 1 }))
    mockEvents.mockResolvedValue({
      events: [{ event_type: "Started", recorded_at: "2026-06-01T01:00:00Z", event_data: null }],
    })
    renderPage()
    await waitFor(() => {
      const timelineLabels = screen.getAllByText("启动")
      expect(timelineLabels.length).toBeGreaterThanOrEqual(1)
    })
  })

  it("renders stream name as clickable link", async () => {
    mockGet.mockResolvedValue(makeTask())
    mockStreamsGet.mockResolvedValue({ id: "stream-1", name: "camera-1" })
    renderPage()
    await waitFor(() => {
      const link = screen.getByText("camera-1").closest("a")
      expect(link?.getAttribute("href")).toBe("/streams/stream-1")
    })
  })

  it("shows stability percentage when frames extracted", async () => {
    mockGet.mockResolvedValue(makeTask({
      status: "Running",
      frames_extracted: 100,
      started_at: new Date(Date.now() - 600000).toISOString(),
    }))
    renderPage()
    await waitFor(() => {
      expect(screen.getByText(/%/)).toBeTruthy()
    })
  })

  it("shows dash for stability when no frames", async () => {
    mockGet.mockResolvedValue(makeTask({ frames_extracted: 0 }))
    renderPage()
    await waitFor(() => {
      const dashes = screen.getAllByText("-")
      expect(dashes.length).toBeGreaterThanOrEqual(1)
    })
  })

  it("renders task info section with ID", async () => {
    mockGet.mockResolvedValue(makeTask())
    renderPage()
    await waitFor(() => {
      expect(screen.getByText("task-1")).toBeTruthy()
    })
  })

  it("deletes task on delete button click", async () => {
    const confirmSpy = vi.spyOn(window, "confirm").mockReturnValue(true)
    mockGet.mockResolvedValue(makeTask({ status: "Stopped" }))
    renderPage()
    await waitFor(() => {
      expect(screen.getByText("删除")).toBeTruthy()
    })
    fireEvent.click(screen.getByText("删除"))
    expect(confirmSpy).toHaveBeenCalled()
    expect(mockDelete).toHaveBeenCalledWith("task-1")
    confirmSpy.mockRestore()
  })
})
