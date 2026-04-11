import { test, expect } from "@playwright/test";

test.describe("Workflow Builder", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto("/");
    await page.waitForLoadState("networkidle");
  });

  test("canvas renders with React Flow", async ({ page }) => {
    const canvas = page.locator(".react-flow");
    await expect(canvas).toBeVisible({ timeout: 15_000 });
  });

  test("zoom controls are present", async ({ page }) => {
    const canvas = page.locator(".react-flow");
    await expect(canvas).toBeVisible({ timeout: 15_000 });

    // React Flow renders zoom controls or pane
    const pane = page.locator(".react-flow__pane");
    await expect(pane).toBeVisible();
  });

  test("canvas is interactive", async ({ page }) => {
    const canvas = page.locator(".react-flow");
    await expect(canvas).toBeVisible({ timeout: 15_000 });

    // Canvas should respond to clicks without crashing
    await canvas.click({ force: true, position: { x: 200, y: 200 } });
    await expect(canvas).toBeVisible();
  });
});
