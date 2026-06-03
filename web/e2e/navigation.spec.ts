import { test, expect } from "@playwright/test"

test.describe("Navigation", () => {
  test.beforeEach(async ({ page }) => {
    await page.route("**/api/v1/**", async (route) => {
      await route.fulfill({ status: 200, contentType: "application/json", body: "{}" })
    })
    await page.goto("/")
  })

  test("navigates to streams page", async ({ page }) => {
    await page.getByRole("link", { name: /流管理/i }).click()
    await expect(page.locator("h1")).toHaveText("流管理")
  })

  test("navigates to tasks page", async ({ page }) => {
    await page.getByRole("link", { name: /任务管理/i }).click()
    await expect(page.locator("h1")).toHaveText("任务管理")
  })

  test("navigates back to dashboard", async ({ page }) => {
    await page.getByRole("link", { name: /流管理/i }).click()
    await page.getByRole("link", { name: /控制面板/i }).click()
    await expect(page.locator("h1")).toHaveText("控制面板")
  })
})
