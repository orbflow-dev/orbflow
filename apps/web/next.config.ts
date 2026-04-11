import type { NextConfig } from "next";

const nextConfig: NextConfig = {
  output: "standalone",
  allowedDevOrigins: ["127.0.0.1", "192.168.1.6"],
  turbopack: {
    // Turbopack is the default bundler in Next.js 16.
    // Use `next dev --webpack` to opt out.
  },
};

export default nextConfig;
