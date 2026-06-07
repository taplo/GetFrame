import { describe, it, expect, vi, beforeEach } from "vitest"
import { render, screen, waitFor, fireEvent } from "@testing-library/react"
import { MemoryRouter } from "react-router-dom"
import RulesPage from "./RulesPage"

const mockRules = vi.fn()
const mockStreams = vi.fn()

vi.mock("@/api/rules", () => ({
  rulesApi: {
    listGlobal: (params?: { stream_id?: string; type?: string }) => mockRules(params),
  },
}))

vi.mock("@/api/streams", () => ({
  streamsApi: {
    list: () => mockStreams(),
  },
}))

function renderPage() {
  return render(
    <MemoryRouter>
      <RulesPage />
    </MemoryRouter>
  )
}

describe("RulesPage", () => {
  beforeEach(() => {
    vi.clearAllMocks()
    mockStreams.mockResolvedValue({ streams: [] })
  })

  it("shows loading state initially", () => {
    mockRules.mockImplementation(() => new Promise(() => {}))
    renderPage()
    expect(screen.getByText("加载中...")).toBeTruthy()
  })

  it("shows empty state when no rules", async () => {
    mockRules.mockResolvedValue({ rules: [] })
    renderPage()
    await waitFor(() => {
      expect(screen.getByText("暂无规则")).toBeTruthy()
    })
  })

  it("renders rules in table", async () => {
    mockRules.mockResolvedValue({
      rules: [
        { stream_id: "id-1", stream_name: "stream-a", source_url: "rtsp://cam1", index: 0, rule: { type: "interval", interval_seconds: 5 } },
        { stream_id: "id-2", stream_name: "stream-b", source_url: "rtsp://cam2", index: 0, rule: { type: "fps", fps: 10 } },
      ],
    })
    renderPage()
    await waitFor(() => {
      expect(screen.getByText("stream-a")).toBeTruthy()
      expect(screen.getByText("stream-b")).toBeTruthy()
    })
    expect(screen.getAllByText("定时抽帧").length).toBeGreaterThanOrEqual(1)
    expect(screen.getAllByText("固定帧率").length).toBeGreaterThanOrEqual(1)
  })

  it("renders rule summary in table", async () => {
    mockRules.mockResolvedValue({
      rules: [
        { stream_id: "id-1", stream_name: "s1", source_url: "", index: 0, rule: { type: "interval", interval_seconds: 5 } },
      ],
    })
    renderPage()
    await waitFor(() => {
      expect(screen.getByText("每 5 秒")).toBeTruthy()
    })
  })

  it("shows error state on API failure", async () => {
    mockRules.mockRejectedValue(new Error("network error"))
    renderPage()
    await waitFor(() => {
      expect(screen.getByText("network error")).toBeTruthy()
    })
    expect(screen.getByText("重试")).toBeTruthy()
  })

  it("retries on retry button click", async () => {
    mockRules.mockRejectedValueOnce(new Error("error")).mockResolvedValueOnce({ rules: [] })
    renderPage()
    await waitFor(() => {
      expect(screen.getByText("重试")).toBeTruthy()
    })
    fireEvent.click(screen.getByText("重试"))
    await waitFor(() => {
      expect(screen.getByText("暂无规则")).toBeTruthy()
    })
  })

  it("passes filter params to API", async () => {
    mockRules.mockResolvedValue({ rules: [] })
    renderPage()
    await waitFor(() => {
      expect(mockRules).toHaveBeenCalledWith({})
    })
  })

  it("renders filter dropdowns", async () => {
    mockRules.mockResolvedValue({ rules: [] })
    renderPage()
    await waitFor(() => {
      expect(screen.getByText("全部流")).toBeTruthy()
      expect(screen.getByText("全部类型")).toBeTruthy()
    })
  })

  it("renders source URL in table", async () => {
    mockRules.mockResolvedValue({
      rules: [
        { stream_id: "id-1", stream_name: "s1", source_url: "rtsp://camera:554/stream", index: 0, rule: { type: "interval", interval_seconds: 5 } },
      ],
    })
    renderPage()
    await waitFor(() => {
      expect(screen.getByText("rtsp://camera:554/stream")).toBeTruthy()
    })
  })

  it("renders rule index in table", async () => {
    mockRules.mockResolvedValue({
      rules: [
        { stream_id: "id-1", stream_name: "s1", source_url: "", index: 2, rule: { type: "interval", interval_seconds: 5 } },
      ],
    })
    renderPage()
    await waitFor(() => {
      expect(screen.getByText("2")).toBeTruthy()
    })
  })
})
