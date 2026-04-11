import { test, expect } from "@playwright/test";

test.describe("Credential Manager", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto("/");
    await page.waitForLoadState("networkidle");
    const nav = page.locator("nav").first();
    await nav.getByText("Credentials").click();
    await page.waitForLoadState("networkidle");
  });

  test("shows credential manager UI", async ({ page }) => {
    await expect(page.locator("body")).toContainText(/credential/i);
  });

  test("credential page renders without errors", async ({ page }) => {
    // Page should not show an error boundary or crash
    const errorBoundary = page.getByText(/something went wrong/i);
    await expect(errorBoundary).not.toBeVisible();

    // The page should have meaningful content
    const body = page.locator("body");
    const text = await body.textContent();
    expect(text!.length).toBeGreaterThan(50);
  });
});
