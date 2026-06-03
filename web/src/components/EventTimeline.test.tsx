import { describe, it, expect } from "vitest"
import { render, screen, fireEvent } from "@testing-library/react"
import { EventTimeline } from "./EventTimeline"
import type { TaskEvent } from "@/api/tasks"

function makeEvent(event_type: string, i: number): TaskEvent {
  return {
    id: `ev-${i}`,
    task_id: "t1",
    event_type,
    recorded_at: `2026-06-03T12:${String(i).padStart(2, "0")}:00Z`,
    event_data: null,
  }
}

function makeEvents(n: number): TaskEvent[] {
  return Array.from({ length: n }, (_, i) => makeEvent("Started", i))
}

describe("EventTimeline", () => {
  it("renders empty state", () => {
    render(<EventTimeline events={[]} />)
    expect(screen.getByText("暂无事件记录")).toBeTruthy()
  })

  it("renders events with default pageSize of 20", () => {
    const events = makeEvents(25)
    render(<EventTimeline events={events} />)
    const items = screen.getAllByText(/启动/)
    expect(items.length).toBe(20)
  })

  it("shows correct event count in load more button", () => {
    const events = makeEvents(25)
    render(<EventTimeline events={events} />)
    expect(screen.getByText("加载更多 (5 条剩余)")).toBeTruthy()
  })

  it("does not show load more when events fit in one page", () => {
    const events = makeEvents(15)
    render(<EventTimeline events={events} />)
    expect(screen.queryByText(/加载更多/)).toBeNull()
  })

  it("hides load more when all events visible", () => {
    const events = makeEvents(20)
    render(<EventTimeline events={events} />)
    expect(screen.queryByText(/加载更多/)).toBeNull()
  })

  it("loads 20 more events on button click", () => {
    const events = makeEvents(45)
    render(<EventTimeline events={events} />)
    expect(screen.getAllByText(/启动/).length).toBe(20)
    fireEvent.click(screen.getByText(/加载更多/))
    expect(screen.getAllByText(/启动/).length).toBe(40)
  })

  it("accepts custom pageSize prop", () => {
    const events = makeEvents(10)
    render(<EventTimeline events={events} pageSize={5} />)
    expect(screen.getAllByText(/启动/).length).toBe(5)
  })

  it("renders different event types with labels", () => {
    const events: TaskEvent[] = [
      makeEvent("Started", 1),
      makeEvent("Paused", 2),
      makeEvent("Resumed", 3),
      makeEvent("Stopped", 4),
      makeEvent("Error", 5),
    ]
    render(<EventTimeline events={events} pageSize={5} />)
    expect(screen.getByText("启动")).toBeTruthy()
    expect(screen.getByText("已暂停")).toBeTruthy()
    expect(screen.getByText("已恢复")).toBeTruthy()
    expect(screen.getByText("已停止")).toBeTruthy()
    expect(screen.getByText("错误")).toBeTruthy()
  })

  it("renders event data message when present", () => {
    const events: TaskEvent[] = [{
      id: "ev-1",
      task_id: "t1",
      event_type: "Error",
      recorded_at: "2026-06-03T12:00:00Z",
      event_data: { message: "connection timeout" },
    }]
    render(<EventTimeline events={events} pageSize={5} />)
    expect(screen.getByText("connection timeout")).toBeTruthy()
  })
})
