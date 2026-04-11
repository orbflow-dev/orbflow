# Builder

The Builder is where you create and edit workflows in Orbflow. It provides a visual canvas where you add steps (called **nodes**), connect them with **edges**, and configure how data flows between them. Think of it as a whiteboard where each sticky note is an action your workflow performs, and the arrows between them define the order of execution.

![Builder](screenshots/01-builder.png)

---

## Canvas workspace

When you open the Builder, you see a large dark canvas that fills most of the screen. This is your workspace. You can:

- **Pan** by clicking and dragging on an empty area of the canvas.
- **Zoom** in and out with your mouse scroll wheel.
- **Select** a node by clicking on it. Selected nodes show a highlighted border.
- **Multi-select** by holding **Shift** and clicking additional nodes, or by holding **Shift** and dragging a selection rectangle around multiple nodes.
- **Right-click** anywhere on the canvas to open a context menu with quick actions.

At the top center of the canvas sits the **toolbar**, and at the bottom center you will find the **workflow selector**. Both float above the canvas so they are always accessible no matter how far you scroll.

---

## Adding nodes

Nodes are the individual steps of your workflow. They are connected by **edges** (the arrows between nodes that define the order of execution). Orbflow gives you three ways to add nodes:

### From a node's "+" button

Hover over any existing node and a small **+** button appears to its right. Click it to open the **node picker**. The new node is automatically placed 150 pixels to the right and connected to the source node with an edge.

### From an edge's "+" button

Hover over any edge (the line between two nodes) and a **+** button appears at the midpoint. Clicking it opens the node picker and inserts the new node in the middle of that edge -- the original connection is split into two new edges so the flow stays intact.

### From the canvas "+" button

When your canvas is empty, a large round **+** button is displayed in the welcome overlay. You can also use the floating add button at the right-center of the canvas. This places a new node at the current position without automatically connecting it.

### The node picker

No matter which method you use, you will see the **node picker** -- a searchable modal that lists every available node type. It includes:

- A **search bar** at the top so you can find nodes by name or description.
- **Category tabs** to filter by type: All, Triggers, Actions, and Connections.
- A **Recently used** section that remembers the last five nodes you added.
- A **4-column grid** of node cards showing each node's icon, name, and description.
- **Keyboard navigation**: use arrow keys to move through the grid, Enter to select, and Escape to close.

---

## Connecting nodes with edges

Edges are the arrows that link one node's output to another node's input.

To create a connection manually, hover over a node until you see its **handles** -- small circles on the sides of the node:

- **Output handle** (right side): drag from here to another node's input handle.
- **Input handle** (left side): this is the drop target for incoming connections.
- **Capability handles** (bottom): some nodes have special connection ports at the bottom for linking to capability-type nodes.

Connection handles are color-coded by data type: green for strings, amber for numbers, purple for booleans, blue for objects, and pink for arrays. You can only connect handles whose types are compatible (or when one side accepts any object type).

When you hover over an edge, you will also see a **trash icon** that lets you delete the connection.

---

## Configuring nodes -- the config modal

Double-click any node (or click on it when it is selected) to open the **config modal**, a full-screen overlay with three panels:

### Left panel -- Available Data

This panel shows all the output fields from upstream nodes that are connected to the current node. It acts as a data browser -- you can see what fields are available and their types. If no upstream nodes are connected, you will see a prompt to connect one first.

You can **click on any field** in this panel to insert it as an expression into the currently active input field in the center panel.

### Center panel -- Parameters

This is where you configure the node's behavior. It contains:

- **Parameters**: settings specific to the node type (for example, the URL and HTTP method for an HTTP Request node).
- **Input Mappings**: fields that accept data from upstream nodes. Each mapping field has a toggle that lets you switch between:
  - **Fixed** mode: enter a literal value directly.
  - **Expression** mode: write a dynamic expression (for example, `=nodes["http_1"].body`) to reference data from other nodes. Orbflow uses a language called CEL (Common Expression Language) for these expressions -- you do not need to learn it up front, since the drag-and-drop mapping writes them for you automatically.
  - **Connected via edge**: when a field is auto-mapped through an edge connection, a badge shows this. You can click "Override" to switch to manual mapping.
- **Settings** tab: some nodes have an additional Settings tab for advanced configuration.
- **Name** field: rename the node to something meaningful (shown in the top-right of the center panel).
- **Requires Approval** toggle: enable this on any action node to pause execution at that step until a human approves it. When enabled, the node displays a shield badge on the canvas.

### Right panel -- Output

Shows the data structure that this node will produce when it runs. Each output field is displayed with its type badge and description. If a field is marked as **dynamic**, its actual structure is only known after execution -- you can click the **Test** button in the header bar to run the node and discover its real output.

### Test button

The header bar of the config modal includes a **Test** button (for non-trigger nodes). Clicking it executes just that single node using its current configuration and upstream data. After the test completes, a green dot appears on the node in the canvas indicating that cached output is available. Downstream nodes can then use this cached output when you configure their input mappings.

Press **Escape**, click the **X** button, or click outside the modal to close it.

---

## Drag-and-drop expression mapping

You can map data between nodes by dragging fields from the left panel (Available Data) and dropping them onto input fields in the center panel (Parameters / Input Mappings).

When you drag a field:

1. A drag preview shows the field name and type.
2. Valid drop targets highlight with an indigo glow.
3. On drop, the input field automatically switches to **Expression** mode and fills in the correct expression path (for example, `nodes["http_1"].body.title`).

This is the fastest way to wire data between steps without manually typing expressions.

---

## Quick-start templates

When you create a new workflow and the canvas is empty, a **welcome overlay** appears with the heading "What would you like to automate?" It offers four pre-built templates you can select to get started instantly:

| Template | Description |
|----------|-------------|
| **API Integration** | Fetches a post from a public API and logs the result. Three nodes: Manual Trigger, HTTP Request, Log. |
| **Scheduled Task** | Waits 3 seconds, calls an API, and logs the response. Four nodes: Manual Trigger, Delay, HTTP Request, Log. |
| **Data Pipeline** | Fetches a list of users, transforms the data, and logs the output. Four nodes: Manual Trigger, HTTP Request, Transform, Log. |
| **Parallel Processing** | Fetches two API endpoints at the same time, then collects results. Four nodes: Manual Trigger, two HTTP Requests, Log. Shows how branching and merging works. |

Click any template card to instantly populate the canvas with pre-connected, pre-configured nodes. Each card shows a preview of the node chain at the bottom so you can see the flow at a glance.

Below the templates, a large **+** button lets you start from scratch, and a row of shortcut hints reminds you of the most common keyboard shortcuts.

---

## Toolbar

The toolbar floats at the top center of the canvas and provides quick access to common actions:

### Workflow name and description

On the left side of the toolbar, click the workflow name to rename it. A small colored dot next to the name indicates save status:
- **Amber dot**: you have unsaved changes.
- **Cyan pulsing dot**: all changes are saved.

Click the description text below the name to add or edit a workflow description.

### Undo and Redo

- **Undo** -- reverts your last change. Grayed out when there is nothing to undo.
- **Redo** -- re-applies a change you just undid. Grayed out when there is nothing to redo.

### Selection actions

- **Duplicate** -- copies the selected nodes and edges. Only available when something is selected.
- **Delete** -- removes the selected nodes and edges. Shown in red to indicate a destructive action.

### Layout tools

- **Auto Layout** -- automatically arranges all nodes in a clean left-to-right layout. Helpful after adding many nodes manually.
- **Zoom to Fit** -- adjusts the zoom level and pan position so that all nodes fit within the visible canvas area.

### Annotations

- **Add Annotation** -- opens a dropdown with two options:
  - **Sticky Note** -- adds a colored note you can write on, useful for documenting parts of your workflow.
  - **Text Label** -- adds a simple text annotation to the canvas.

### View controls

- **Search Nodes** (Ctrl+F) -- opens a search overlay to find nodes on the canvas by name.
- **Snap to Grid** (Ctrl+G) -- toggles grid snapping on and off. When active, nodes snap to an invisible grid as you drag them, helping you keep a tidy layout. The button glows indigo when grid snapping is on.
- **Keyboard Shortcuts** (?) -- opens a searchable reference dialog listing all available shortcuts.

### Stats

The toolbar shows a live count of nodes and edges in your workflow (for example, "3n . 2e"). When multiple items are selected, this changes to show the selection count (for example, "5 selected").

### Version History

If available, a clock icon opens the version history panel where you can browse and restore previous versions of your workflow.

### Save and Run

- **Save** -- saves the current workflow to the server. Shows a spinning indicator while saving.
- **Run** -- executes the workflow. The button appearance adapts based on your trigger type:
  - **Manual trigger**: a purple "Run" button.
  - **Webhook trigger**: shows the webhook path with a copy button, plus a "Test" button to trigger a test run.
  - **Cron trigger**: shows the schedule in human-readable form (for example, "Every 5 min") with an Active/Inactive toggle and a "Test" button.
  - **Event trigger**: shows the event name with a "Test" button.

---

## Keyboard shortcuts

Press **?** at any time to open the full keyboard shortcuts reference. Here are the most important ones:

### General

| Shortcut | Action |
|----------|--------|
| Ctrl+S | Save workflow |
| Ctrl+Enter | Run workflow |
| Escape | Deselect all / Close open panel |
| ? | Show keyboard shortcuts help |

### Editing

| Shortcut | Action |
|----------|--------|
| Ctrl+Z | Undo |
| Ctrl+Shift+Z | Redo |
| Ctrl+D | Duplicate selected |
| Ctrl+C | Copy selected |
| Ctrl+V | Paste |
| Ctrl+X | Cut selected |
| Ctrl+A | Select all nodes and edges |
| Delete | Delete selected |

### Canvas

| Shortcut | Action |
|----------|--------|
| Scroll wheel | Zoom in / out |
| Click + drag | Pan canvas |
| Shift + drag | Area select |
| Shift + click | Add to selection |
| Ctrl+F | Search nodes |
| Ctrl+G | Toggle snap to grid |
| Right-click | Open context menu |

On macOS, use Cmd instead of Ctrl. The shortcuts dialog automatically shows the correct key symbols for your platform.

---

## Workflow selector

At the bottom center of the canvas, the **workflow selector** lets you switch between workflows or create a new one. It shows the name of the currently active workflow.

- Click the selector to open a **dropdown** listing all your saved workflows, with a search bar to filter by name.
- Select **New workflow** at the top of the list to start a fresh canvas.
- Click any workflow name to load it onto the canvas.
- Use the **Import** button (upload icon) to load a workflow from a JSON file.
- Use the **Export** button (download icon) to save the current workflow as a JSON file for backup or sharing.

---

## Node types at a glance

On the canvas, different node types have distinct visual shapes so you can quickly identify their role:

| Type | Shape | Purpose |
|------|-------|---------|
| **Trigger** | Rounded square with a lightning bolt badge | The starting point of a workflow. Has only an output handle (right side). |
| **Action** | Rounded square | Performs work (HTTP requests, transformations, logging, etc.). Has input (left) and output (right) handles. |
| **Capability** | Circle | A specialized connection node (databases, services). Has only an input handle (top). |

Each node displays its icon and name below it. If you rename a node, the custom name appears as a subtitle beneath the original type name.
