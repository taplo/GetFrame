import { test, expect } from "@playwright/test"

test.describe("Dashboard", () => {
  test.beforeEach(async ({ page }) => {
    const now = new Date().toISOString()
    await page.route("**/api/v1/streams**", async (route) => {
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({
          streams: [
            { id: "1", name: "Camera-1", status: "online", frames_per_hour: 3600, source_url: "rtsp://...", source_type: "rtsp", tags: {}, description: "", latest_frame_key: null, last_online: now, last_error: null, error_count: 0, uptime_seconds: 3600, frames_decoded: 1000, frames_extracted: 500, reconnect_count: 0, created_at: now },
            { id: "2", name: "Camera-2", status: "online", frames_per_hour: 1800, source_url: "rtsp://...", source_type: "rtsp", tags: {}, description: "", latest_frame_key: null, last_online: now, last_error: null, error_count: 0, uptime_seconds: 7200, frames_decoded: 2000, frames_extracted: 1000, reconnect_count: 0, created_at: now },
            { id: "3", name: "Camera-3", status: "error", frames_per_hour: 0, source_url: "rtsp://...", source_type: "rtsp", tags: {}, description: "", latest_frame_key: null, last_online: null, last_error: "Connection refused", error_count: 5, uptime_seconds: 0, frames_decoded: 0, frames_extracted: 0, reconnect_count: 3, created_at: now },
          ],
        }),
      })
    })
    await page.route("**/api/v1/tasks**", async (route) => {
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({
          tasks: [
            { id: "t1", name: "Task-A", stream_id: "1", stream_name: "Camera-1", status: "Running", rules: [{ type: "interval", params: { interval_seconds: 5 } }], frames_extracted: 1500, created_at: now },
            { id: "t2", name: "Task-B", stream_id: "2", stream_name: "Camera-2", status: "Paused", rules: [{ type: "interval", params: { interval_seconds: 10 } }], frames_extracted: 500, created_at: now },
          ],
        }),
      })
    })
    await page.route("**/api/v1/metrics/history**", async (route) => {
      const points = Array.from({ length: 30 }, (_, i) => ({
        recorded_at: new Date(Date.now() - i * 60000).toISOString(),
        streams_active: 2,
        frames_ps: 5.0 + Math.random(),
        kafka_ps: 4.8 + Math.random(),
        errors_decode: 0,
        errors_storage: 0,
        errors_kafka: 0,
        streams_claimed: 2,
      }))
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({ points }),
      })
    })
    await page.goto("/")
  })

  test("displays page title", async ({ page }) => {
    await expect(page.locator("h1")).toHaveText("控制面板")
  })

  test("shows stat cards with correct values", async ({ page }) => {
    await expect(page.getByText("在线流")).toBeVisible()
    await expect(page.getByText("活跃任务")).toBeVisible()
    await expect(page.getByText("抽帧总数")).toBeVisible()
    await expect(page.getByText("离线流")).toBeVisible()
  })

  test("shows 2 online streams and 1 error stream", async ({ page }) => {
    await expect(page.getByText("Camera-1")).toBeVisible()
    await expect(page.getByText("Camera-2")).toBeVisible()
    await expect(page.getByText("Camera-3")).toBeVisible()
  })

  test("shows running and paused tasks", async ({ page }) => {
    await expect(page.getByText("Task-A")).toBeVisible()
    await expect(page.getByText("Task-B")).toBeVisible()
  })

  test("SystemHealth component renders", async ({ page }) => {
    await expect(page.getByText("系统状态")).toBeVisible()
  })

  test("MetricsChart renders with kafka panel", async ({ page }) => {
    await expect(page.getByText(/最近30分钟/)).toBeVisible()
  })
})
