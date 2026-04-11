# Marketplace

The Marketplace is your gateway to extending Orbflow with community plugins and integrations. Browse a curated catalog of plugins that add new node types to the Builder, install them with a single click, and manage everything from one place.

![Marketplace](screenshots/06-marketplace.png)

---

## Navigating the Marketplace

When you open the Marketplace from the sidebar, you'll see a header area with controls at the top and a scrollable plugin grid below. The header shows the total number of available plugins and gives you quick access to search, sort, filter, and submit your own plugins.

### Browse and Installed Tabs

At the top of the page, two tabs let you switch between views:

- **Browse** -- Shows all plugins available in the community registry, whether or not you've installed them. This is the default view.
- **Installed** -- Filters the list to show only plugins you have currently installed. Use this to review what's active in your workspace or to uninstall plugins you no longer need.

The active tab is highlighted in indigo. Switching tabs preserves your current search query and category filter.

### Sorting Plugins

A dropdown next to the tabs lets you control the order of results:

- **Name A-Z** -- Alphabetical order (default).
- **Name Z-A** -- Reverse alphabetical order.
- **Most Downloads** -- Sorts by popularity, showing the most-downloaded plugins first.

### Search Bar

Below the tabs, a search bar lets you find plugins by keyword. Type a plugin name, description fragment, or tool name and results update automatically as you type (with a brief debounce so the list doesn't flicker while you're still typing).

**Keyboard shortcut:** Press `/` anywhere on the page to jump straight to the search bar.

### Category Filters

A row of category buttons sits below the search bar. Click any category to narrow the plugin list:

| Category | What it includes |
|---|---|
| **All** | Every plugin, regardless of category |
| **AI** | Language models, classification, extraction, sentiment analysis |
| **Database** | Database connectors and query tools |
| **Communication** | Email, chat, messaging, and notification integrations |
| **Utility** | General-purpose tools -- parsing, encoding, transformation |
| **Monitoring** | Observability, alerting, and health-check plugins |
| **Security** | Scanning, secrets management, and compliance tools |
| **Cloud** | Cloud provider integrations (AWS, GCP, Azure, etc.) |
| **Integration** | Connectors for third-party SaaS platforms |

The active category is highlighted in indigo. Click **All** to clear the filter and see every plugin again.

---

## Browsing Plugins

### Featured Plugins

When you first land on the Browse tab with no search or category filter active, the top of the grid shows a **Featured** section. This highlights the first four plugins in a larger two-column card layout so you can quickly discover popular or noteworthy additions.

Below the featured section, an **All Plugins** heading introduces the full grid.

### Plugin Grid

Plugins are displayed as cards in a responsive grid -- one column on small screens, scaling up to four columns on wide displays. Each card shows:

- **Icon** -- A colored icon representing the plugin's category (violet for AI, emerald for Database, sky-blue for Communication, and so on).
- **Name** -- The plugin's display name.
- **Description** -- A short summary of what the plugin does (up to two lines).
- **Version** -- The latest published version (e.g., `v0.2.1`).
- **Downloads** -- How many times the plugin has been downloaded (formatted as `1.2k`, `3.5M`, etc.).
- **Author** -- Who created the plugin.
- **Install status** -- A green "Installed" badge if you already have it, or an amber "Update" badge when a newer version is available.
- **Category badge** -- The plugin's category in a colored label.
- **Tags** -- Up to two keyword tags for quick context (e.g., `csv`, `parser`).

Click any card to open the plugin detail panel.

### Pagination

When there are more plugins than fit on a single page (20 per page), pagination controls appear at the bottom. Use **Previous** and **Next** to move between pages, and a page indicator shows your current position (e.g., `2 / 5`).

---

## Plugin Details

Clicking a plugin card opens a detail panel that slides in from the right side of the screen. Press **Escape** or click the backdrop to close it.

The detail panel contains:

- **Hero section** -- A larger icon, the plugin name, full description, version, author, and download count, all styled with a gradient background that matches the plugin's category.
- **Metadata grid** -- Three at-a-glance fields: License, Minimum Orbflow Version, and Language.
- **Repository link** -- A direct link to the plugin's source code on GitHub (opens in a new tab).
- **Nodes Provided** -- A list of node type identifiers the plugin registers. These are the new node types that will appear in the Builder after installation.
- **Tags** -- The full set of keyword tags.
- **README** -- If the plugin includes a README, it is displayed here in a scrollable monospace block.
- **Install / Uninstall buttons** -- See below.

---

## Installing and Uninstalling Plugins

### Installing a Plugin

1. Open the plugin's detail panel by clicking its card.
2. Click the **Install Plugin** button at the bottom of the panel.
3. A progress indicator shows the stages: Downloading, Extracting, Verifying, and finally Complete.
4. Once installed, a green "Installed" banner replaces the install button.

After installation, the plugin's node types are immediately available in the Builder. Open the node picker (the **+** button on any node or the canvas) and you'll find the new node types listed alongside the built-in ones. No restart is required.

### Uninstalling a Plugin

1. Open the detail panel for an installed plugin.
2. Click the **Uninstall** button beneath the green "Installed" banner.
3. A confirmation prompt appears asking you to confirm. Click **Confirm Uninstall** to proceed, or **Cancel** to keep the plugin.
4. The plugin files are removed and its node types are no longer available in the Builder.

---

## Submitting a Plugin

If you've built your own plugin, the **Submit Plugin** button in the top-right corner of the Marketplace walks you through a three-step wizard.

### Step 1 -- Prerequisites

Before proceeding, confirm that your plugin meets these requirements:

- It is hosted on a **public GitHub repository**.
- The repository contains an `orbflow-plugin.json` manifest file.
- It implements the Orbflow plugin protocol (**gRPC**, a high-performance communication protocol, or **Subprocess**, a simpler process-based protocol).
- It has been **tested and working locally**.

Links to the Community Plugin Registry and Plugin Development Guide are provided for reference.

### Step 2 -- Plugin Details

Fill in a form with your plugin's information:

| Field | Description |
|---|---|
| Plugin Name | A unique identifier (e.g., `orbflow-my-plugin`) |
| Version | Semantic version (e.g., `0.1.0`) |
| Description | A short summary of what your plugin does |
| Author | Your name or organization |
| Category | One of: AI, Database, Communication, Utility, Monitoring, Security, Cloud, Integration |
| Node Types | Comma-separated list of node type IDs your plugin registers (e.g., `sql_query, sql_execute`) |
| Protocol | gRPC or Subprocess |
| GitHub Repo | `owner/repo-name` format |
| Path in Repo | For monorepos, the subdirectory containing the plugin |
| Tags | Comma-separated keywords |
| Language | Python, TypeScript, Rust, or Go |
| License | e.g., MIT, Apache-2.0 |
| Min Orbflow Version | Minimum compatible Orbflow version |
| Icon | Icon name from the Orbflow icon set |
| Color | Hex color code for branding |

Click **Validate & Continue** to check your entry for errors. Any issues are shown inline so you can correct them before proceeding.

### Step 3 -- Submit to Registry

The wizard generates a JSON entry for your plugin. To publish it:

1. Click **Copy** to copy the JSON to your clipboard.
2. Click **Open on GitHub** to navigate to the `plugins.json` file in the community registry.
3. Add your entry to the array and submit a Pull Request.
4. Once the PR is reviewed and merged, your plugin appears in the Marketplace for all users.

---

## Using Installed Plugins in the Builder

Installed plugins register new node types with Orbflow's schema registry. This means they show up alongside built-in nodes whenever you add a node to a workflow:

- Click the **+** button on an existing node, on an edge, or on the canvas to open the node picker.
- Search or browse to find the plugin's node types.
- Add the node to your workflow and configure it just like any built-in node -- the plugin defines its own input fields, parameters, and outputs through the standard schema system.

Installed plugins are marked as "active and available in the node picker" in the detail panel, confirming they're ready to use.
