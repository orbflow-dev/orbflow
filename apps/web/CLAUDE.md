# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Development Commands

```bash
pnpm dev      # Next.js dev server (http://localhost:3000)
pnpm build    # Production build (standalone output)
pnpm lint     # Run Next.js linting
```

The backend API URL defaults to `http://localhost:8080` and is configurable via `NEXT_PUBLIC_API_URL`.

## Architecture

Visual workflow builder for the Orbflow workflow engine. Built with Next.js 16 (Turbopack), React 19, TypeScript, TailwindCSS 4, Zustand 5, and @xyflow/react 12. Single-page app with four tabs: Builder, Activity, Templates, Credentials.

### Module Boundaries

Strict separation between an **embeddable core** and **application shell**:

```
src/
├── core/           ← Embeddable workflow builder (public API via core/index.ts)
│   ├── types/           Domain types (FieldSchema, NodeTypeDefinition, FieldMapping, ConditionGroup)
│   ├── schemas/         NodeSchemaRegistry + built-in node definitions
│   ├── components/      Canvas nodes/edges, config modal, picker popover, toolbar
│   │   ├── canvas/          WorkflowNode, ConditionalEdge, TextAnnotationNode, StickyNoteNode
│   │   ├── config-panel/    MappingField, FieldBrowser, NodeConfig (sidebar fallback)
│   │   ├── node-picker-popover.tsx   Floating searchable node selection
│   │   └── node-config-modal.tsx     3-panel config modal (INPUT | PARAMS | OUTPUT)
│   ├── context/         OrbflowProvider + ThemeProvider (dark/light mode)
│   ├── styles/          OrbflowTheme interface + default dark/light themes
│   └── utils/           CEL builder, upstream resolution, auto-layout, node-slug generation
├── components/     ← App-specific pages consuming core's public API
│   ├── workflow-builder/   Workflow selector + builder wrapper
│   ├── execution-viewer/   Instance list + status monitor + execution graph
│   ├── credential-manager/ Credential CRUD manager + credential selector dropdown
│   └── template-gallery/   Pre-built workflow templates
├── store/          ← Zustand stores (7 stores, no cross-imports between them)
│   ├── canvas-store.ts     Nodes/edges/selection/annotations/capabilityEdges
│   ├── panel-store.ts      Input mappings, edge conditions, parameter values
│   ├── picker-store.ts     Node picker popover position/state
│   ├── history-store.ts    Undo/redo snapshots (max 50)
│   ├── workflow-store.ts   Backend workflows/instances CRUD
│   ├── credential-store.ts Credential CRUD with toast feedback
│   └── toast-store.ts      Notification queue with auto-dismiss
├── lib/api.ts      ← HTTP client — expects { data, error } response envelope
└── app/
    ├── layout.tsx          Root layout (Sora + JetBrains Mono fonts)
    ├── page.tsx            Main app frame: sidebar nav, tabbed content, OrbflowProvider
    └── globals.css         Tailwind 4 theme, animations, React Flow overrides
```

### Data Flow

1. **Canvas → Store**: xyflow events → `canvasStore.onNodesChange()/onEdgesChange()`
2. **Config Modal → Store**: User edits → `panelStore.setInputMapping()/setParameterValue()`
3. **Save**: `buildWorkflowPayload()` collects canvas nodes/edges + panel mappings/conditions → serialized to backend format
4. **CEL Convention**: Values prefixed with `=` are CEL expressions (e.g., `=nodes["http-1"].body`), others are literals

### Node System

Node types are defined via `NodeTypeDefinition` with typed `inputs`/`outputs` (`FieldSchema[]`). The `NodeSchemaRegistry` holds all registered types. Built-in schemas in `core/schemas/builtin.ts`; additional schemas load from `/node-types` API.

**Three node kinds** with distinct visual representations in `WorkflowNode`:

| Kind | Shape | Size | Handles | Visual |
|------|-------|------|---------|--------|
| `trigger` | Rounded square | 68px | Output (right) only | Red lightning badge, tinted bg |
| `action` | Rounded square | 64px | Input (left) + Output (right) + Cap ports (bottom) | Neutral bg, "+" hover button |
| `capability` | Circle | 52px | Input (top) only, **no output** | Sky-blue border, smaller icon |

**Input mapping modes** (per field):
- **Static (Fixed)**: literal value, toggled via "Fixed" button
- **Expression**: CEL expression with `fx` badge, toggled via "Expression" button
- **Wired**: auto-mapped when connecting handles; shows "Connected via edge" badge with Override option

### Node Insertion & Canvas Interactions

Nodes are added via a **picker popover** (not a static sidebar). Three insertion paths:

1. **From node "+"**: Opens picker at node's right edge → new node placed 150px right → auto-connected
2. **From edge "+"**: Opens picker at edge midpoint → edge split (old edge removed, two new edges created)
3. **From canvas "+"**: Floating button at right-center → node placed at flow position, **no modal auto-open**

The config modal only auto-opens when the new node is connected (has sourceNodeId or sourceEdgeId).

**Edge hover interactions** (`ConditionalEdge`): invisible 20px wide hit-area shows "+" (insert node) and trash (delete edge) buttons at the edge midpoint on hover.

### Config Modal (n8n-style 3-panel)

`node-config-modal.tsx` — portal to `document.body`, full-screen overlay:

```
┌─────────────────────────────────────────────────────┐
│  [Icon] Node Name                    Name: [___] [X] │
├──────────────┬──────────────────┬───────────────────┤
│  INPUT       │  Parameters |    │  OUTPUT           │
│  (280px)     │  Settings        │  (280px)          │
│              │  (flex center)   │                   │
│  Upstream    │  ParameterField  │  Node output      │
│  FieldBrowser│  MappingField    │  schema fields    │
│  (draggable) │  (drop targets)  │                   │
└──────────────┴──────────────────┴───────────────────┘
```

- Tabs use `#FF6D5A` active indicator
- Escape / click-outside / X button to close
- Each field has a **Fixed/Expression segmented toggle** (n8n pattern)

### Drag-and-Drop Expression Mapping

Fields in the FieldBrowser (left panel) are draggable. MappingField and ParameterField are drop targets.

- MIME type: `application/orbflow-field`
- Payload: `{ nodeId, path, celPath }` (JSON)
- CEL path format: `nodes["nodeId"].field.subfield` for upstream, `vars` or `trigger` for context
- On drop: field switches to expression mode with the CEL path auto-filled
- Drop highlight: `ring-1 ring-electric-indigo/40 bg-electric-indigo/[0.04]`

### Canvas Conventions

- Node IDs: `node_${Date.now()}`, Edge IDs: `edge_${Date.now()}`
- Edge splitting uses `_a` / `_b` suffixes to avoid ID collisions
- Custom xyflow node type: `"task"` → `WorkflowNode`
- Custom xyflow edge type: `"conditional"` → `ConditionalEdge`
- Handle colors by type: string=green, number=amber, boolean=purple, object=blue, array=pink
- Connection validation: types must match (or one side is `"object"`)

### Portal Pattern

Overlay components render via `createPortal(jsx, document.body)` for z-index management:
- `NodePickerPopover` — z-[90], viewport-clamped positioning
- `NodeConfigModal` — z-[80], backdrop blur + slide-up animation
- Toast notifications — portal to body

### Styling

- TailwindCSS 4 with `@tailwindcss/postcss` plugin
- Custom theme tokens in `globals.css`: obsidian (#0A0A0C), charcoal (#141417), electric-indigo (#7C5CFC), neon-cyan (#22D3EE), port-* type colors
- Fonts: Sora (UI) and JetBrains Mono (code/expressions)
- Key animations: `modalBackdropIn`, `modalSlideUp`, `edgeMenuIn`, `flow-dash`, `fadeInUp`, `scaleIn`
- Handle hover: no `transform: scale` (causes xyflow positioning bugs) — use background/box-shadow/border-color transitions only
- `.orbflow-add-btn`: "+" button positioned at `right: -28px`, hidden until `.orbflow-node:hover`
- `src/core/components/icons.tsx`: 40+ SVG icons (stroke-based, 24x24)

### Theme System

`ThemeProvider` (`core/context/theme-provider.tsx`) wraps the app with dark/light mode support. Persists preference to `localStorage` key `"orbflow-theme"` and sets `data-theme` attribute on `<html>`. Hook: `useTheme()` returns `{ mode, toggleTheme, setMode }`. Theme tokens defined as CSS custom properties (`--orbflow-*`) in `globals.css` with both `:root` (dark) and `[data-theme="light"]` variants. TypeScript theme shapes in `core/styles/theme.ts`.

### Credential System (Frontend)

Full CRUD for encrypted credentials. `credential-store.ts` manages state with toast notifications on actions. `credential-manager.tsx` is a two-pane layout (list sidebar + form/details). `credential-selector.tsx` is a dropdown for referencing credentials in node config. FieldSchema supports `type: "credential"` with optional `credentialType` filter (e.g., `"smtp"`, `"postgres"`). API endpoints: `/credentials` (CRUD), `/credential-types` (schema listing).

### Text Annotations & Sticky Notes

`TextAnnotationNode` allows freeform text labels on the canvas (double-click to edit, auto-focus). `StickyNoteNode` for visual notes. Canvas store tracks annotations separately: `addAnnotation()`, `removeAnnotation()`, `updateAnnotation()`, `updateAnnotationStyle()`.

### Execution Graph

`execution-graph.tsx` visualizes workflow execution status using ReactFlow (readonly). Custom `executionNode` type with status-colored badges (green=completed, red=failed, cyan=running, gray=pending, amber=cancelled). Animated edges for running nodes. Stats bar shows completion counts. Selected node panel displays error messages and JSON output.

### Utilities

- `src/lib/cn.ts`: `cn()` — `clsx` + `tailwind-merge` for className merging (shadcn pattern)
- `src/lib/use-focus-trap.ts`: React hook to trap Tab focus within a container (modals/popovers)
- `src/core/utils/node-slug.ts`: `generateNodeSlug()` — converts schema name to kebab-case slug with auto-increment (e.g., `http_request_1`, `http_request_2`)

### Path Alias

`@/*` maps to `./src/*` (configured in tsconfig.json).
