import { describe, it, expect, vi, beforeEach } from "vitest"
import { render, screen, waitFor, fireEvent } from "@testing-library/react"
import { MemoryRouter, Route, Routes } from "react-router-dom"
import StreamDetail from "./StreamDetail"

const mockGet = vi.fn()
const mockDelete = vi.fn()

vi.mock("@/api/streams", () => ({
  streamsApi: {
    get: (id: string) => mockGet(id),
    delete: (id: string) => mockDelete(id),
  },
}))

vi.mock("@/components/FramePreview", () => ({
  FramePreview: ({ streamId }: { streamId: string }) => <div data-testid="frame-preview">{streamId}</div>,
}))

function renderPage(id: string = "test-id-1234") {
  return render(
    <MemoryRouter initialEntries={[`/streams/${id}`]}>
      <Routes>
        <Route path="/streams/:id" element={<StreamDetail />} />
        <Route path="/streams" element={<div data-testid="streams-list">Streams List</div>} />
      </Routes>
    </MemoryRouter>
  )
}

describe("StreamDetail", () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it("shows loading state initially", () => {
    mockGet.mockImplementation(() => new Promise(() => {}))
    renderPage()
    expect(screen.getByText("加载中...")).toBeTruthy()
  })

  it("renders stream info when loaded", async () => {
    mockGet.mockResolvedValue({
      id: "test-id-1234",
      name: "test-camera",
      source_url: "rtsp://192.168.1.1:554/stream",
      source_type: "rtsp",
      status: "online",
      frames_decoded: 1500,
      frames_extracted: 300,
      frames_per_hour: 0,
      uptime_seconds: 7200,
      error_count: 0,
      tags: [],
      description: "",
      last_online: null,
      last_error: null,
      reconnect_count: 0,
      latest_frame_key: null,
      created_at: "2026-06-01T00:00:00Z",
    })
    renderPage()
    await waitFor(() => {
      expect(screen.getByText("test-camera")).toBeTruthy()
    })
    expect(screen.getByText("在线")).toBeTruthy()
    expect(screen.getByText("rtsp://192.168.1.1:554/stream")).toBeTruthy()
    expect(screen.getByText("1500")).toBeTruthy()
    expect(screen.getByText("300")).toBeTruthy()
  })

  it("renders offline status", async () => {
    mockGet.mockResolvedValue({
      id: "test-id", name: "cam", source_url: "", source_type: "file",
      status: "offline", frames_decoded: 0, frames_extracted: 0,
      frames_per_hour: 0, uptime_seconds: 0, error_count: 0,
      tags: [], description: "", last_online: null, last_error: null,
      reconnect_count: 0, latest_frame_key: null, created_at: "",
    })
    renderPage()
    await waitFor(() => {
      expect(screen.getByText("离线")).toBeTruthy()
    })
  })

  it("renders error status", async () => {
    mockGet.mockResolvedValue({
      id: "test-id", name: "cam", source_url: "", source_type: "file",
      status: "error: timeout", frames_decoded: 0, frames_extracted: 0,
      frames_per_hour: 0, uptime_seconds: 0, error_count: 0,
      tags: [], description: "", last_online: null, last_error: null,
      reconnect_count: 0, latest_frame_key: null, created_at: "",
    })
    renderPage()
    await waitFor(() => {
      expect(screen.getByText("error: timeout")).toBeTruthy()
    })
  })

  it("navigates to streams list on not found", async () => {
    mockGet.mockRejectedValue(new Error("not found"))
    renderPage()
    await waitFor(() => {
      expect(screen.getByTestId("streams-list")).toBeTruthy()
    })
  })

  it("renders back button", async () => {
    mockGet.mockResolvedValue({
      id: "test-id", name: "cam", source_url: "", source_type: "file",
      status: "online", frames_decoded: 0, frames_extracted: 0,
      frames_per_hour: 0, uptime_seconds: 0, error_count: 0,
      tags: [], description: "", last_online: null, last_error: null,
      reconnect_count: 0, latest_frame_key: null, created_at: "",
    })
    renderPage()
    await waitFor(() => {
      expect(screen.getByText("← 返回流列表")).toBeTruthy()
    })
  })

  it("renders edit and delete buttons", async () => {
    mockGet.mockResolvedValue({
      id: "test-id", name: "cam", source_url: "", source_type: "file",
      status: "online", frames_decoded: 0, frames_extracted: 0,
      frames_per_hour: 0, uptime_seconds: 0, error_count: 0,
      tags: [], description: "", last_online: null, last_error: null,
      reconnect_count: 0, latest_frame_key: null, created_at: "",
    })
    renderPage()
    await waitFor(() => {
      expect(screen.getByText("编辑")).toBeTruthy()
      expect(screen.getByText("删除")).toBeTruthy()
    })
  })

  it("deletes stream and navigates to list", async () => {
    const confirmSpy = vi.spyOn(window, "confirm").mockReturnValue(true)
    mockGet.mockResolvedValue({
      id: "test-id-1234", name: "cam", source_url: "", source_type: "file",
      status: "online", frames_decoded: 0, frames_extracted: 0,
      frames_per_hour: 0, uptime_seconds: 0, error_count: 0,
      tags: [], description: "", last_online: null, last_error: null,
      reconnect_count: 0, latest_frame_key: null, created_at: "",
    })
    mockDelete.mockResolvedValue(undefined)
    renderPage()
    await waitFor(() => {
      expect(screen.getByText("删除")).toBeTruthy()
    })
    fireEvent.click(screen.getByText("删除"))
    expect(confirmSpy).toHaveBeenCalled()
    expect(mockDelete).toHaveBeenCalledWith("test-id-1234")
    await waitFor(() => {
      expect(screen.getByTestId("streams-list")).toBeTruthy()
    })
    confirmSpy.mockRestore()
  })

  it("renders frame preview after load", async () => {
    mockGet.mockResolvedValue({
      id: "test-id", name: "cam", source_url: "", source_type: "file",
      status: "online", frames_decoded: 0, frames_extracted: 0,
      frames_per_hour: 0, uptime_seconds: 0, error_count: 0,
      tags: [], description: "", last_online: null, last_error: null,
      reconnect_count: 0, latest_frame_key: null, created_at: "",
    })
    renderPage()
    await waitFor(() => {
      expect(screen.getByTestId("frame-preview")).toBeTruthy()
    })
  })
})
