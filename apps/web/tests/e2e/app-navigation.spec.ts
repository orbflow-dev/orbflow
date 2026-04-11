import { test, expect } from "@playwright/test";

test.describe("App Navigation", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto("/");
    await page.waitForLoadState("networkidle");
  });

  test("app loads and shows sidebar navigation", async ({ page }) => {
    // Sidebar should be visible with nav items
    const nav = page.locator("nav").first();
    await expect(nav).toBeVisible();

    // Nav items should exist within sidebar
    await expect(nav.getByText("Builder")).toBeVisible();
    await expect(nav.getByText("Activity")).toBeVisible();
    await expect(nav.getByText("Templates")).toBeVisible();
    await expect(nav.getByText("Credentials")).toBeVisible();
  });

  test("defaults to builder tab", async ({ page }) => {
    // Builder tab should be active by default
    const builderBtn = page.getByText("Builder").first();
    await expect(builderBtn).toBeVisible();
  });

  test("can navigate to Activity tab", async ({ page }) => {
    const nav = page.locator("nav").first();
    await nav.getByRole("tab", { name: "Activity" }).click();
    await page.waitForLoadState("networkidle");

    // Activity tab content should be visible
    await expect(page.locator("body")).toContainText(/activity|execution|instance|run/i);
  });

  test("can navigate to Templates tab", async ({ page }) => {
    const nav = page.locator("nav").first();
    await nav.getByRole("tab", { name: "Templates" }).click();
    await page.waitForLoadState("networkidle");

    // Templates content should load
    await expect(page.locator("body")).toContainText(/template/i);
  });

  test("can navigate to Credentials tab", async ({ page }) => {
    const nav = page.locator("nav").first();
    await nav.getByRole("tab", { name: "Credentials" }).click();
    await page.waitForLoadState("networkidle");

    // Credentials content should load
    await expect(page.locator("body")).toContainText(/credential/i);
  });

  test("theme toggle switches between dark and light", async ({ page }) => {
    // App starts in dark mode by default
    const html = page.locator("html");

    // Find and click theme toggle
    const themeBtn = page.getByLabel(/switch to light mode|switch to dark mode/i);
    await expect(themeBtn).toBeVisible();

    // Get initial theme
    const initialTheme = await html.getAttribute("data-theme");

    // Toggle theme
    await themeBtn.click();

    // Theme should change
    const newTheme = await html.getAttribute("data-theme");
    expect(newTheme).not.toBe(initialTheme);

    // Toggle back
    await themeBtn.click();
    const restoredTheme = await html.getAttribute("data-theme");
    expect(restoredTheme).toBe(initialTheme);
  });
});
