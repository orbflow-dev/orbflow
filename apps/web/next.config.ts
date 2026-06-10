import type { NextConfig } from "next";

// SHA-256 of the themeInitScript inline in src/app/layout.tsx.
// Update this hash whenever the script body changes.
const THEME_INIT_SCRIPT_SHA256 =
  "sha256-zFAavkW2xVlgSfp1rMNtnfCm+Dub9Wzk3osmYUA1LHo=";

// Backend API origin for connect-src, resolved at build time. Must be an
// origin (scheme://host[:port]) — a trailing path would invalidate the CSP source.
const API_ORIGIN = process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:8080";

const nextConfig: NextConfig = {
  output: "standalone",
  allowedDevOrigins: ["127.0.0.1", "192.168.1.6"],
  turbopack: {
    // Turbopack is the default bundler in Next.js 16.
    // Use `next dev --webpack` to opt out.
  },
  async headers() {
    return [
      {
        source: "/(.*)",
        headers: [
          { key: "X-Frame-Options", value: "DENY" },
          { key: "X-Content-Type-Options", value: "nosniff" },
          { key: "Referrer-Policy", value: "strict-origin-when-cross-origin" },
          { key: "X-DNS-Prefetch-Control", value: "off" },
          {
            key: "Strict-Transport-Security",
            value: "max-age=63072000; includeSubDomains",
          },
          {
            // Report-Only: violations are reported but NOT enforced.
            // Promote to Content-Security-Policy once nonce middleware exists (T-12).
            // style-src 'unsafe-inline' is required for xyflow injected styles.
            // connect-src includes the backend API (NEXT_PUBLIC_API_URL default).
            key: "Content-Security-Policy-Report-Only",
            value: [
              "default-src 'self'",
              `script-src 'self' '${THEME_INIT_SCRIPT_SHA256}'`,
              "style-src 'self' 'unsafe-inline'",
              "img-src 'self' data: blob:",
              `connect-src 'self' ${API_ORIGIN}`,
              "frame-ancestors 'none'",
            ].join("; "),
          },
        ],
      },
    ];
  },
};

export default nextConfig;
