import type { SVGProps, ReactElement } from "react";

type IconComponent = (props: SVGProps<SVGSVGElement>) => ReactElement;

// Stroke-based icon set — consistent 24x24 viewBox, strokeWidth 1.5
const s = { viewBox: "0 0 24 24", fill: "none", stroke: "currentColor", strokeWidth: 1.5, strokeLinecap: "round" as const, strokeLinejoin: "round" as const };

const iconMap: Record<string, IconComponent> = {
  globe: (p) => (
    <svg {...s} {...p}><circle cx="12" cy="12" r="10"/><path d="M2 12h20"/><path d="M12 2a15.3 15.3 0 014 10 15.3 15.3 0 01-4 10 15.3 15.3 0 01-4-10A15.3 15.3 0 0112 2z"/></svg>
  ),
  clock: (p) => (
    <svg {...s} {...p}><circle cx="12" cy="12" r="10"/><polyline points="12 6 12 12 16 14"/></svg>
  ),
  terminal: (p) => (
    <svg {...s} {...p}><polyline points="4 17 10 11 4 5"/><line x1="12" y1="19" x2="20" y2="19"/></svg>
  ),
  workflow: (p) => (
    <svg {...s} {...p}><rect x="3" y="3" width="7" height="7" rx="1.5"/><rect x="14" y="3" width="7" height="7" rx="1.5"/><rect x="8.5" y="14" width="7" height="7" rx="1.5"/><path d="M6.5 10v1.5a1 1 0 001 1h9a1 1 0 001-1V10"/><path d="M12 12.5V14"/></svg>
  ),
  mail: (p) => (
    <svg {...s} {...p}><rect x="2" y="4" width="20" height="16" rx="2"/><path d="M22 7l-10 7L2 7"/></svg>
  ),
  users: (p) => (
    <svg {...s} {...p}><path d="M16 21v-2a4 4 0 00-4-4H6a4 4 0 00-4 4v2"/><circle cx="9" cy="7" r="4"/><path d="M22 21v-2a4 4 0 00-3-3.87"/><path d="M16 3.13a4 4 0 010 7.75"/></svg>
  ),
  database: (p) => (
    <svg {...s} {...p}><ellipse cx="12" cy="5" rx="9" ry="3"/><path d="M21 12c0 1.66-4 3-9 3s-9-1.34-9-3"/><path d="M3 5v14c0 1.66 4 3 9 3s9-1.34 9-3V5"/></svg>
  ),
  filter: (p) => (
    <svg {...s} {...p}><polygon points="22 3 2 3 10 12.46 10 19 14 21 14 12.46 22 3"/></svg>
  ),
  zap: (p) => (
    <svg {...s} {...p}><polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2"/></svg>
  ),
  "file-text": (p) => (
    <svg {...s} {...p}><path d="M14 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V8z"/><polyline points="14 2 14 8 20 8"/><line x1="16" y1="13" x2="8" y2="13"/><line x1="16" y1="17" x2="8" y2="17"/><polyline points="10 9 9 9 8 9"/></svg>
  ),
  "bar-chart": (p) => (
    <svg {...s} {...p}><line x1="12" y1="20" x2="12" y2="10"/><line x1="18" y1="20" x2="18" y2="4"/><line x1="6" y1="20" x2="6" y2="16"/></svg>
  ),
  bell: (p) => (
    <svg {...s} {...p}><path d="M18 8A6 6 0 006 8c0 7-3 9-3 9h18s-3-2-3-9"/><path d="M13.73 21a2 2 0 01-3.46 0"/></svg>
  ),
  shield: (p) => (
    <svg {...s} {...p}><path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/></svg>
  ),
  code: (p) => (
    <svg {...s} {...p}><polyline points="16 18 22 12 16 6"/><polyline points="8 6 2 12 8 18"/></svg>
  ),
  "git-branch": (p) => (
    <svg {...s} {...p}><line x1="6" y1="3" x2="6" y2="15"/><circle cx="18" cy="6" r="3"/><circle cx="6" cy="18" r="3"/><path d="M18 9a9 9 0 01-9 9"/></svg>
  ),
  repeat: (p) => (
    <svg {...s} {...p}><polyline points="17 1 21 5 17 9"/><path d="M3 11V9a4 4 0 014-4h14"/><polyline points="7 23 3 19 7 15"/><path d="M21 13v2a4 4 0 01-4 4H3"/></svg>
  ),
  cloud: (p) => (
    <svg {...s} {...p}><path d="M18 10h-1.26A8 8 0 109 20h9a5 5 0 000-10z"/></svg>
  ),
  link: (p) => (
    <svg {...s} {...p}><path d="M10 13a5 5 0 007.54.54l3-3a5 5 0 00-7.07-7.07l-1.72 1.71"/><path d="M14 11a5 5 0 00-7.54-.54l-3 3a5 5 0 007.07 7.07l1.71-1.71"/></svg>
  ),
  settings: (p) => (
    <svg {...s} {...p}><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 00.33 1.82l.06.06a2 2 0 010 2.83 2 2 0 01-2.83 0l-.06-.06a1.65 1.65 0 00-1.82-.33 1.65 1.65 0 00-1 1.51V21a2 2 0 01-4 0v-.09A1.65 1.65 0 009 19.4a1.65 1.65 0 00-1.82.33l-.06.06a2 2 0 01-2.83 0 2 2 0 010-2.83l.06-.06A1.65 1.65 0 004.68 15a1.65 1.65 0 00-1.51-1H3a2 2 0 010-4h.09A1.65 1.65 0 004.6 9a1.65 1.65 0 00-.33-1.82l-.06-.06a2 2 0 012.83-2.83l.06.06A1.65 1.65 0 009 4.68a1.65 1.65 0 001-1.51V3a2 2 0 014 0v.09a1.65 1.65 0 001 1.51 1.65 1.65 0 001.82-.33l.06-.06a2 2 0 012.83 2.83l-.06.06A1.65 1.65 0 0019.4 9a1.65 1.65 0 001.51 1H21a2 2 0 010 4h-.09a1.65 1.65 0 00-1.51 1z"/></svg>
  ),
  package: (p) => (
    <svg {...s} {...p}><line x1="16.5" y1="9.4" x2="7.5" y2="4.21"/><path d="M21 16V8a2 2 0 00-1-1.73l-7-4a2 2 0 00-2 0l-7 4A2 2 0 003 8v8a2 2 0 001 1.73l7 4a2 2 0 002 0l7-4A2 2 0 0021 16z"/><polyline points="3.27 6.96 12 12.01 20.73 6.96"/><line x1="12" y1="22.08" x2="12" y2="12"/></svg>
  ),
  send: (p) => (
    <svg {...s} {...p}><line x1="22" y1="2" x2="11" y2="13"/><polygon points="22 2 15 22 11 13 2 9 22 2"/></svg>
  ),
  inbox: (p) => (
    <svg {...s} {...p}><polyline points="22 12 16 12 14 15 10 15 8 12 2 12"/><path d="M5.45 5.11L2 12v6a2 2 0 002 2h16a2 2 0 002-2v-6l-3.45-6.89A2 2 0 0016.76 4H7.24a2 2 0 00-1.79 1.11z"/></svg>
  ),
  layers: (p) => (
    <svg {...s} {...p}><polygon points="12 2 2 7 12 12 22 7 12 2"/><polyline points="2 17 12 22 22 17"/><polyline points="2 12 12 17 22 12"/></svg>
  ),
  "arrow-right": (p) => (
    <svg {...s} {...p}><line x1="5" y1="12" x2="19" y2="12"/><polyline points="12 5 19 12 12 19"/></svg>
  ),
  check: (p) => (
    <svg {...s} {...p}><polyline points="20 6 9 17 4 12"/></svg>
  ),
  x: (p) => (
    <svg {...s} {...p}><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
  ),
  plus: (p) => (
    <svg {...s} {...p}><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
  ),
  search: (p) => (
    <svg {...s} {...p}><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>
  ),
  "chevron-down": (p) => (
    <svg {...s} {...p}><polyline points="6 9 12 15 18 9"/></svg>
  ),
  "chevron-right": (p) => (
    <svg {...s} {...p}><polyline points="9 18 15 12 9 6"/></svg>
  ),
  play: (p) => (
    <svg {...s} {...p}><polygon points="5 3 19 12 5 21 5 3"/></svg>
  ),
  save: (p) => (
    <svg {...s} {...p}><path d="M19 21H5a2 2 0 01-2-2V5a2 2 0 012-2h11l5 5v11a2 2 0 01-2 2z"/><polyline points="17 21 17 13 7 13 7 21"/><polyline points="7 3 7 8 15 8"/></svg>
  ),
  "help-circle": (p) => (
    <svg {...s} {...p}><circle cx="12" cy="12" r="10"/><path d="M9.09 9a3 3 0 015.83 1c0 2-3 3-3 3"/><line x1="12" y1="17" x2="12.01" y2="17"/></svg>
  ),
  undo: (p) => (
    <svg {...s} {...p}><polyline points="1 4 1 10 7 10"/><path d="M3.51 15a9 9 0 102.13-9.36L1 10"/></svg>
  ),
  redo: (p) => (
    <svg {...s} {...p}><polyline points="23 4 23 10 17 10"/><path d="M20.49 15a9 9 0 11-2.12-9.36L23 10"/></svg>
  ),
  copy: (p) => (
    <svg {...s} {...p}><rect x="9" y="9" width="13" height="13" rx="2"/><path d="M5 15H4a2 2 0 01-2-2V4a2 2 0 012-2h9a2 2 0 012 2v1"/></svg>
  ),
  trash: (p) => (
    <svg {...s} {...p}><polyline points="3 6 5 6 21 6"/><path d="M19 6v14a2 2 0 01-2 2H7a2 2 0 01-2-2V6m3 0V4a2 2 0 012-2h4a2 2 0 012 2v2"/></svg>
  ),
  "auto-layout": (p) => (
    <svg {...s} {...p}><rect x="3" y="3" width="7" height="5" rx="1"/><rect x="14" y="3" width="7" height="5" rx="1"/><rect x="8.5" y="16" width="7" height="5" rx="1"/><line x1="6.5" y1="8" x2="6.5" y2="12"/><line x1="17.5" y1="8" x2="17.5" y2="12"/><line x1="6.5" y1="12" x2="17.5" y2="12"/><line x1="12" y1="12" x2="12" y2="16"/></svg>
  ),
  "zoom-fit": (p) => (
    <svg {...s} {...p}><path d="M15 3h6v6"/><path d="M9 21H3v-6"/><path d="M21 3l-7 7"/><path d="M3 21l7-7"/></svg>
  ),
  "grip-vertical": (p) => (
    <svg {...s} {...p}><circle cx="9" cy="5" r="1" fill="currentColor" stroke="none"/><circle cx="15" cy="5" r="1" fill="currentColor" stroke="none"/><circle cx="9" cy="12" r="1" fill="currentColor" stroke="none"/><circle cx="15" cy="12" r="1" fill="currentColor" stroke="none"/><circle cx="9" cy="19" r="1" fill="currentColor" stroke="none"/><circle cx="15" cy="19" r="1" fill="currentColor" stroke="none"/></svg>
  ),
  "alert-triangle": (p) => (
    <svg {...s} {...p}><path d="M10.29 3.86L1.82 18a2 2 0 001.71 3h16.94a2 2 0 001.71-3L13.71 3.86a2 2 0 00-3.42 0z"/><line x1="12" y1="9" x2="12" y2="13"/><line x1="12" y1="17" x2="12.01" y2="17"/></svg>
  ),
  upload: (p) => (
    <svg {...s} {...p}><path d="M21 15v4a2 2 0 01-2 2H5a2 2 0 01-2-2v-4"/><polyline points="17 8 12 3 7 8"/><line x1="12" y1="3" x2="12" y2="15"/></svg>
  ),
  download: (p) => (
    <svg {...s} {...p}><path d="M21 15v4a2 2 0 01-2 2H5a2 2 0 01-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>
  ),
  clipboard: (p) => (
    <svg {...s} {...p}><path d="M16 4h2a2 2 0 012 2v14a2 2 0 01-2 2H6a2 2 0 01-2-2V6a2 2 0 012-2h2"/><rect x="8" y="2" width="8" height="4" rx="1"/></svg>
  ),
  "message-square": (p) => (
    <svg {...s} {...p}><path d="M21 15a2 2 0 01-2 2H7l-4 4V5a2 2 0 012-2h14a2 2 0 012 2z"/></svg>
  ),
  webhook: (p) => (
    <svg {...s} {...p}><path d="M18 16.98h-5.99c-1.1 0-1.95.94-2.48 1.9A4 4 0 012 17c.01-.7.2-1.4.57-2"/><path d="M6 17l3.13-5.78c.53-.97.1-2.18-.5-3.1a4 4 0 113.5-2.02"/><path d="M12 6l3.13 5.73c.53.98 1.74 1.28 2.74.95A4 4 0 0121 17c-.46.68-1.08 1.22-1.8 1.57"/></svg>
  ),
  "arrow-up-down": (p) => (
    <svg {...s} {...p}><path d="M7 3l-4 4h3v7H3l4 4 4-4H8V7h3L7 3z" transform="translate(2,1)"/><path d="M17 21l4-4h-3V10h3l-4-4-4 4h3v7h-3l4 4z" transform="translate(-2,-1)"/></svg>
  ),
  plug: (p) => (
    <svg {...s} {...p}><path d="M12 22v-5"/><path d="M9 8V2"/><path d="M15 8V2"/><path d="M18 8v5a6 6 0 01-12 0V8z"/></svg>
  ),
  sun: (p) => (
    <svg {...s} {...p}><circle cx="12" cy="12" r="5"/><line x1="12" y1="1" x2="12" y2="3"/><line x1="12" y1="21" x2="12" y2="23"/><line x1="4.22" y1="4.22" x2="5.64" y2="5.64"/><line x1="18.36" y1="18.36" x2="19.78" y2="19.78"/><line x1="1" y1="12" x2="3" y2="12"/><line x1="21" y1="12" x2="23" y2="12"/><line x1="4.22" y1="19.78" x2="5.64" y2="18.36"/><line x1="18.36" y1="5.64" x2="19.78" y2="4.22"/></svg>
  ),
  moon: (p) => (
    <svg {...s} {...p}><path d="M21 12.79A9 9 0 1111.21 3 7 7 0 0021 12.79z"/></svg>
  ),
  "sticky-note": (p) => (
    <svg {...s} {...p}><path d="M15.5 3H5a2 2 0 00-2 2v14c0 1.1.9 2 2 2h14a2 2 0 002-2V8.5L15.5 3z"/><path d="M14 3v7h7"/></svg>
  ),
  type: (p) => (
    <svg {...s} {...p}><polyline points="4 7 4 4 20 4 20 7"/><line x1="9" y1="20" x2="15" y2="20"/><line x1="12" y1="4" x2="12" y2="20"/></svg>
  ),
  square: (p) => (
    <svg {...s} {...p}><rect x="3" y="3" width="18" height="18" rx="2"/></svg>
  ),
  circle: (p) => (
    <svg {...s} {...p}><circle cx="12" cy="12" r="10"/></svg>
  ),
  diamond: (p) => (
    <svg {...s} {...p}><rect x="3.5" y="3.5" width="17" height="17" rx="1" transform="rotate(45 12 12)"/></svg>
  ),
  frame: (p) => (
    <svg {...s} {...p}><rect x="2" y="2" width="20" height="20" rx="2" strokeDasharray="4 2"/></svg>
  ),
  "file-code": (p) => (
    <svg {...s} {...p}><path d="M14 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V8z"/><polyline points="14 2 14 8 20 8"/><path d="M10 12l-2 2 2 2"/><path d="M14 12l2 2-2 2"/></svg>
  ),
  key: (p) => (
    <svg {...s} {...p}><path d="M21 2l-2 2m-7.61 7.61a5.5 5.5 0 11-7.78 7.78 5.5 5.5 0 017.78-7.78zm0 0L15.5 7.5m0 0l3 3L22 7l-3-3m-3.5 3.5L19 4"/></svg>
  ),
  loader: (p) => (
    <svg {...s} {...p}><path d="M12 2v4"/><path d="M12 18v4"/><path d="M4.93 4.93l2.83 2.83"/><path d="M16.24 16.24l2.83 2.83"/><path d="M2 12h4"/><path d="M18 12h4"/><path d="M4.93 19.07l2.83-2.83"/><path d="M16.24 7.76l2.83-2.83"/></svg>
  ),
  default: (p) => (
    <svg {...s} {...p}><polygon points="12 2 22 8.5 22 15.5 12 22 2 15.5 2 8.5 12 2"/><circle cx="12" cy="12" r="3"/></svg>
  ),
};

export function NodeIcon({
  name,
  ...props
}: { name: string } & SVGProps<SVGSVGElement>) {
  const Icon = iconMap[name] || iconMap.default;
  return <Icon {...props} />;
}

// Type color mapping for port handles
export const TYPE_COLORS: Record<string, string> = {
  string: "#10B981",
  number: "#F59E0B",
  boolean: "#A855F7",
  object: "#3B82F6",
  array: "#EC4899",
};

export function getTypeColor(type: string): string {
  return TYPE_COLORS[type] || "#71717A";
}

// Friendly type labels for non-technical users
export const TYPE_LABELS: Record<string, string> = {
  string: "Text",
  number: "Number",
  boolean: "Yes/No",
  object: "Data",
  array: "List",
};

export function getTypeLabel(type: string): string {
  return TYPE_LABELS[type] || type;
}
