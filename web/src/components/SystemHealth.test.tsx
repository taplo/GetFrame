import { describe, it, expect, vi, beforeEach } from "vitest"
import { render, screen, waitFor } from "@testing-library/react"
import { SystemHealth } from "./SystemHealth"

const mockHealth = vi.fn()
const mockReady = vi.fn()

vi.mock("@/api/health", () => ({
  healthApi: {
    health: () => mockHealth(),
    ready: () => mockReady(),
  },
}))

describe("SystemHealth", () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it("renders loading skeleton initially", () => {
    mockHealth.mockImplementation(() => new Promise(() => {}))
    mockReady.mockImplementation(() => new Promise(() => {}))
    const { container } = render(<SystemHealth />)
    expect(container.querySelector(".animate-pulse")).toBeTruthy()
  })

  it("renders healthy state", async () => {
    mockHealth.mockResolvedValue({ status: "healthy", uptime_seconds: 3600, version: "0.2.0", active_streams: 5 })
    mockReady.mockResolvedValue({ ready: true })
    render(<SystemHealth />)
    await waitFor(() => {
      expect(screen.getByText("系统 健康")).toBeTruthy()
    })
    expect(screen.getByText(/1h 0m/)).toBeTruthy()
    expect(screen.getByText(/v0.2.0/)).toBeTruthy()
    expect(screen.getByText("就绪")).toBeTruthy()
  })

  it("renders degraded state when not ready", async () => {
    mockHealth.mockResolvedValue({ status: "healthy", uptime_seconds: 0, version: "0.2.0", active_streams: 0 })
    mockReady.mockResolvedValue({ ready: false })
    render(<SystemHealth />)
    await waitFor(() => {
      expect(screen.getByText("系统 降级")).toBeTruthy()
    })
    expect(screen.getByText("未就绪")).toBeTruthy()
  })

  it("renders unhealthy state on connection failure", async () => {
    mockHealth.mockRejectedValue(new Error("connection refused"))
    render(<SystemHealth />)
    await waitFor(() => {
      expect(screen.getByText("系统 无法连接")).toBeTruthy()
    })
  })

  it("renders unhealthy state when health returns non-healthy", async () => {
    mockHealth.mockResolvedValue({ status: "error", uptime_seconds: 0, version: "", active_streams: 0 })
    mockReady.mockResolvedValue({ ready: true })
    render(<SystemHealth />)
    await waitFor(() => {
      expect(screen.getByText("系统 故障")).toBeTruthy()
    })
  })

  it("polls every 10 seconds", async () => {
    vi.useFakeTimers()
    mockHealth.mockResolvedValue({ status: "healthy", uptime_seconds: 0, version: "0.2.0", active_streams: 0 })
    mockReady.mockResolvedValue({ ready: true })
    render(<SystemHealth />)
    await vi.advanceTimersByTimeAsync(10000)
    expect(mockHealth).toHaveBeenCalledTimes(2)
    vi.useRealTimers()
  })
})
