import { describe, it, expect } from "vitest";
import nextConfig from "../../next.config";

// Security contract test for the HTTP response headers defined in next.config.ts.
// These assertions form a regression net for the headers() implementation:
// any removal, rename, or value change in the security headers will be caught here.

describe("nextConfig.headers() — security contract", () => {
  it("returns exactly one route covering all paths", async () => {
    const routes = await nextConfig.headers!();
    expect(routes).toHaveLength(1);
    expect(routes[0].source).toBe("/(.*)");
  });

  it("sets X-Frame-Options to DENY", async () => {
    const routes = await nextConfig.headers!();
    const route = routes[0];
    const value = route.headers.find((h) => h.key === "X-Frame-Options")?.value;
    expect(value).toBe("DENY");
  });

  it("sets X-Content-Type-Options to nosniff", async () => {
    const routes = await nextConfig.headers!();
    const route = routes[0];
    const value = route.headers.find((h) => h.key === "X-Content-Type-Options")?.value;
    expect(value).toBe("nosniff");
  });

  it("sets Referrer-Policy to strict-origin-when-cross-origin", async () => {
    const routes = await nextConfig.headers!();
    const route = routes[0];
    const value = route.headers.find((h) => h.key === "Referrer-Policy")?.value;
    expect(value).toBe("strict-origin-when-cross-origin");
  });

  it("sets X-DNS-Prefetch-Control to off", async () => {
    const routes = await nextConfig.headers!();
    const route = routes[0];
    const value = route.headers.find((h) => h.key === "X-DNS-Prefetch-Control")?.value;
    expect(value).toBe("off");
  });

  it("sets Strict-Transport-Security with max-age and includeSubDomains", async () => {
    const routes = await nextConfig.headers!();
    const route = routes[0];
    const value = route.headers.find((h) => h.key === "Strict-Transport-Security")?.value;
    expect(value).toBeDefined();
    expect(value).toContain("max-age=");
    expect(value).toContain("includeSubDomains");
    // preload submits the domain to the irreversible browser preload list —
    // adding it must be a deliberate decision, never an accidental edit.
    expect(value).not.toContain("preload");
  });

  it("includes the backend API origin in connect-src (env-driven, localhost fallback)", async () => {
    const routes = await nextConfig.headers!();
    const route = routes[0];
    const value = route.headers.find((h) => h.key === "Content-Security-Policy-Report-Only")?.value;
    expect(value).toBeDefined();
    const expectedOrigin = process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:8080";
    expect(value).toContain(`connect-src 'self' ${expectedOrigin}`);
  });

  it("sets Content-Security-Policy-Report-Only with default-src 'self'", async () => {
    const routes = await nextConfig.headers!();
    const route = routes[0];
    const value = route.headers.find((h) => h.key === "Content-Security-Policy-Report-Only")?.value;
    expect(value).toBeDefined();
    expect(value).toContain("default-src 'self'");
  });

  it("sets Content-Security-Policy-Report-Only with frame-ancestors 'none'", async () => {
    const routes = await nextConfig.headers!();
    const route = routes[0];
    const value = route.headers.find((h) => h.key === "Content-Security-Policy-Report-Only")?.value;
    expect(value).toBeDefined();
    expect(value).toContain("frame-ancestors 'none'");
  });

  it("sets Content-Security-Policy-Report-Only with the themeInitScript sha256 hash in script-src", async () => {
    const routes = await nextConfig.headers!();
    const route = routes[0];
    const value = route.headers.find((h) => h.key === "Content-Security-Policy-Report-Only")?.value;
    expect(value).toBeDefined();
    expect(value).toContain("script-src");
    expect(value).toContain("sha256-zFAavkW2xVlgSfp1rMNtnfCm+Dub9Wzk3osmYUA1LHo=");
  });

  // Risk #3 regression net: this is the most critical assertion.
  // next.config.ts intentionally uses Report-Only mode (T-12 note).
  // If someone accidentally promotes to the enforcing header name, this catches it.
  it("does NOT include an enforcing Content-Security-Policy header in any route", async () => {
    const routes = await nextConfig.headers!();
    for (const route of routes) {
      const enforcing = route.headers.find((h) => h.key === "Content-Security-Policy");
      expect(enforcing).toBeUndefined();
    }
  });
});
