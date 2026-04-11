import type { Metadata } from "next";
import { Sora, JetBrains_Mono } from "next/font/google";
import "./globals.css";

const sora = Sora({
  subsets: ["latin"],
  display: "swap",
  variable: "--font-sora",
});

const jetbrainsMono = JetBrains_Mono({
  subsets: ["latin"],
  display: "swap",
  variable: "--font-mono",
});

export const metadata: Metadata = {
  title: "Orbflow - Visual Workflow Automation",
  description:
    "Build powerful automations visually. No code required. Connect your tools, automate your processes.",
  icons: {
    icon: "/favicon.svg",
  },
};

const themeInitScript = `
  (() => {
    try {
      const stored = localStorage.getItem("orbflow-theme");
      const resolved = stored === "light" || stored === "dark" ? stored : "dark";
      document.documentElement.setAttribute("data-theme", resolved);
    } catch {
      document.documentElement.setAttribute("data-theme", "dark");
    }
  })();
`;

export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <html
      lang="en"
      data-theme="dark"
      className={`${sora.variable} ${jetbrainsMono.variable}`}
      suppressHydrationWarning
    >
      <head>
        <meta name="color-scheme" content="dark light" />
        <script dangerouslySetInnerHTML={{ __html: themeInitScript }} />
      </head>
      <body className="antialiased">{children}</body>
    </html>
  );
}
