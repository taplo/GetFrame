import { describe, it, expect } from "vitest"
import { render, screen } from "@testing-library/react"
import { MetricsChart } from "./MetricsChart"
import type { MetricsPoint } from "@/types/metrics"

function makePoint(overrides: Partial<MetricsPoint> = {}): MetricsPoint {
  return {
    recorded_at: "2026-06-03T12:00:00Z",
    streams_active: 10,
    frames_ps: 5.0,
    errors_decode: 0,
    errors_storage: 0,
    errors_kafka: 0,
    kafka_ps: 2.5,
    streams_claimed: 8,
    ...overrides,
  }
}

describe("MetricsChart", () => {
  it("renders empty state when no data", () => {
    render(<MetricsChart points={[]} />)
    expect(screen.getByText("暂无指标数据")).toBeTruthy()
  })

  it("renders 4 panels when data provided", () => {
    const points = [makePoint(), makePoint({ recorded_at: "2026-06-03T12:01:00Z" })]
    const { container } = render(<MetricsChart points={points} />)
    const panels = container.querySelectorAll(".bg-white.border.rounded-xl")
    expect(panels.length).toBe(4)
  })

  it("renders active streams panel", () => {
    render(<MetricsChart points={[makePoint()]} />)
    expect(screen.getByText("活跃流趋势")).toBeTruthy()
  })

  it("renders frame rate panel", () => {
    render(<MetricsChart points={[makePoint()]} />)
    expect(screen.getByText("抽帧速率")).toBeTruthy()
  })

  it("renders error rate panel", () => {
    render(<MetricsChart points={[makePoint()]} />)
    expect(screen.getByText("错误率（60s 窗口）")).toBeTruthy()
  })

  it("renders kafka delivery rate panel", () => {
    render(<MetricsChart points={[makePoint()]} />)
    expect(screen.getByText("Kafka 投递率")).toBeTruthy()
  })

  it("formats kafka rate with one decimal", () => {
    const points = [
      makePoint({ recorded_at: "2026-06-03T12:00:00Z", kafka_ps: 2.5678 }),
    ]
    render(<MetricsChart points={points} />)
    expect(screen.getByText("Kafka 投递率")).toBeTruthy()
  })

  it("renders grid layout with 2 columns", () => {
    const { container } = render(<MetricsChart points={[makePoint(), makePoint()]} />)
    const grid = container.querySelector(".grid.grid-cols-2")
    expect(grid).toBeTruthy()
  })
})
