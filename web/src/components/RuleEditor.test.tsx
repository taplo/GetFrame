import { describe, it, expect } from "vitest"
import { render, screen, fireEvent } from "@testing-library/react"
import { RuleEditor } from "./RuleEditor"
import type { RuleConfig } from "@/types/rule"

describe("RuleEditor", () => {
  it("renders all rule type buttons", () => {
    render(<RuleEditor rules={[]} onChange={() => {}} />)
    expect(screen.getByText("定时抽帧")).toBeTruthy()
    expect(screen.getByText("固定帧率")).toBeTruthy()
    expect(screen.getByText("场景变化")).toBeTruthy()
    expect(screen.getByText("限速")).toBeTruthy()
    expect(screen.getByText("复合规则")).toBeTruthy()
  })

  it("defaults to interval type with param 5", () => {
    render(<RuleEditor rules={[]} onChange={() => {}} />)
    expect(screen.getByDisplayValue("5")).toBeTruthy()
    expect(screen.getByText("间隔（秒）")).toBeTruthy()
  })

  it("switches param label when type changes", () => {
    render(<RuleEditor rules={[]} onChange={() => {}} />)
    fireEvent.click(screen.getByText("固定帧率"))
    expect(screen.getByText("FPS")).toBeTruthy()
    expect(screen.getByDisplayValue("10")).toBeTruthy()
  })

  it("switches to threshold param for scene change", () => {
    render(<RuleEditor rules={[]} onChange={() => {}} />)
    fireEvent.click(screen.getByText("场景变化"))
    expect(screen.getByText("阈值 (0.0~1.0)")).toBeTruthy()
    expect(screen.getByDisplayValue("0.3")).toBeTruthy()
  })

  it("switches to rate limit param", () => {
    render(<RuleEditor rules={[]} onChange={() => {}} />)
    fireEvent.click(screen.getByText("限速"))
    expect(screen.getByText("每分钟上限")).toBeTruthy()
    expect(screen.getByDisplayValue("30")).toBeTruthy()
  })

  it("hides param input for composite type", () => {
    render(<RuleEditor rules={[]} onChange={() => {}} />)
    fireEvent.click(screen.getByText("复合规则"))
    expect(screen.queryByText("间隔（秒）")).toBeNull()
    expect(screen.queryByText("FPS")).toBeNull()
    expect(screen.queryByText("阈值 (0.0~1.0)")).toBeNull()
  })

  it("calls onChange when adding a rule", () => {
    const onChange = vi.fn()
    render(<RuleEditor rules={[]} onChange={onChange} />)
    fireEvent.click(screen.getByText("添加规则"))
    expect(onChange).toHaveBeenCalledTimes(1)
    const rules = onChange.mock.calls[0][0] as RuleConfig[]
    expect(rules).toHaveLength(1)
    expect(rules[0].type).toBe("interval")
    expect(rules[0].interval_seconds).toBe(5)
  })

  it("displays existing rules with summary", () => {
    const rules: RuleConfig[] = [
      { type: "interval", interval_seconds: 10 },
      { type: "fps", fps: 25 },
    ]
    render(<RuleEditor rules={rules} onChange={() => {}} />)
    expect(screen.getByText(/每 10 秒/)).toBeTruthy()
    expect(screen.getByText(/25 FPS/)).toBeTruthy()
  })

  it("displays correct count for existing rules", () => {
    const rules: RuleConfig[] = [
      { type: "interval", interval_seconds: 5 },
      { type: "fps", fps: 10 },
    ]
    render(<RuleEditor rules={rules} onChange={() => {}} />)
    expect(screen.getByText("已添加规则 (2)")).toBeTruthy()
  })

  it("calls onChange with remaining rules when removing a rule", () => {
    const rules: RuleConfig[] = [
      { type: "interval", interval_seconds: 5 },
      { type: "fps", fps: 10 },
      { type: "scene_change", threshold: 0.3 },
    ]
    const onChange = vi.fn()
    render(<RuleEditor rules={rules} onChange={onChange} />)
    const deleteBtns = screen.getAllByText("删除")
    fireEvent.click(deleteBtns[1])
    expect(onChange).toHaveBeenCalledTimes(1)
    const remaining = onChange.mock.calls[0][0] as RuleConfig[]
    expect(remaining).toHaveLength(2)
    expect(remaining[0].type).toBe("interval")
    expect(remaining[1].type).toBe("scene_change")
  })

  it("adds rule with custom param value", () => {
    const onChange = vi.fn()
    render(<RuleEditor rules={[]} onChange={onChange} />)
    const input = screen.getByDisplayValue("5") as HTMLInputElement
    fireEvent.change(input, { target: { value: "15" } })
    fireEvent.click(screen.getByText("添加规则"))
    const rules = onChange.mock.calls[0][0] as RuleConfig[]
    expect(rules[0].interval_seconds).toBe(15)
  })

  it("renders scene_change rule summary with threshold", () => {
    const rules: RuleConfig[] = [{ type: "scene_change", threshold: 0.5 }]
    render(<RuleEditor rules={rules} onChange={() => {}} />)
    expect(screen.getByText(/阈值 0.5/)).toBeTruthy()
  })

  it("renders rate_limited rule summary", () => {
    const rules: RuleConfig[] = [{ type: "rate_limited", rule: { type: "interval", interval_seconds: 5 }, max_per_minute: 60 }]
    render(<RuleEditor rules={rules} onChange={() => {}} />)
    expect(screen.getByText(/限速 60\/分钟/)).toBeTruthy()
  })

  it("renders composite rule summary", () => {
    const rules: RuleConfig[] = [{ type: "composite", operator: "any", rules: [] }]
    render(<RuleEditor rules={rules} onChange={() => {}} />)
    expect(screen.getByText(/复合 \(any\)/)).toBeTruthy()
  })
})
