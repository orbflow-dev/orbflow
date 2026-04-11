# Templates

The Templates page gives you a library of pre-built workflows you can use as starting points. Instead of building every workflow from scratch, pick a template that matches your goal, and Orbflow will set up the nodes, edges, and configuration for you automatically.

![Templates](screenshots/04-templates.png)

---

## Template Gallery

When you open the Templates tab from the sidebar, you'll see a grid of template cards. The gallery displays all available templates by default, arranged in a responsive grid that adapts to your screen size -- one column on narrow screens, two on medium, and three on wide displays.

At the top of the page, a header shows the total number of pre-built workflows available (currently 10).

---

## Searching for Templates

A search bar sits at the top of the gallery. Type any keyword to filter templates by name, description, use case, or even individual node names within a template.

- Press `/` on your keyboard to jump straight to the search bar from anywhere on the page.
- A small `x` button appears inside the search bar when you have text entered -- click it to clear your search.
- A result count appears below the filters whenever a search or category filter is active (for example, "Showing 3 templates for 'API'").

---

## Category Filters

Below the search bar, a row of category pills lets you narrow the gallery to a specific type of workflow. Each pill shows its name and the number of templates it contains.

| Category | What it covers |
|----------|---------------|
| **All Templates** | Shows every template in the gallery |
| **Integration** | Templates that connect external APIs and webhooks |
| **Automation** | Templates with timed delays, scheduled actions, and process flows |
| **Data** | Templates for fetching, transforming, filtering, and sorting data |
| **Security** | Templates demonstrating encoding, hashing, and data transformation |
| **AI / ML** | Templates that use AI nodes for summarization, sentiment analysis, and more |
| **Notification** | Templates for sending emails and alerts |

Click any category to filter the grid. The active category is highlighted with an indigo accent. Click **All Templates** to return to the full list.

If your search and category combination returns no results, an empty state appears with a **Clear all filters** link to reset everything.

---

## Template Cards

Each template is displayed as a card containing the following information:

- **Icon and name** -- A colored icon representing the template's category, alongside the template name (for example, "API Integration" or "Data Pipeline").
- **Use case tag** -- A short label describing the template's purpose, such as "Connect APIs", "Timed actions", or "AI Processing".
- **Difficulty indicator** -- A colored dot next to a label showing how complex the template is:
  - Green dot -- **Beginner**: straightforward workflows with a few steps.
  - Amber dot -- **Intermediate**: workflows involving data transformation, AI processing, or multi-step logic.
  - Red dot -- **Advanced**: workflows with parallel branches or fan-out/fan-in patterns.
- **Description** -- One to two sentences explaining what the workflow does and what concepts it demonstrates.
- **Mini flow preview** -- A small inline diagram showing the workflow's node-and-edge structure. This gives you a quick visual sense of whether the workflow is a simple chain, a branching pattern, or something more complex.
- **Node chain** -- A list of the first few node names in execution order (for example, "Manual Trigger > Fetch Product > Log Response"), so you can see what steps the workflow includes at a glance.
- **Node and edge counts** -- At the bottom of the card, small counters show how many nodes and edges the template contains (for example, "3 nodes, 2 edges").

---

## Using a Template

Each card has a **Use template** button in the bottom-right corner. When you click it, one of two things happens:

### If the Builder canvas is empty

Orbflow creates a new workflow from the template immediately. The workflow is saved to the server with the template's name and description, and you are switched to the **Builder** tab where the nodes and edges are ready for you to customize.

### If the Builder canvas already has a workflow

A confirmation dialog appears with the message:

> **Replace current workflow?**
> The Builder has an existing workflow. Using this template will replace it with a new workflow.

You have two choices:

- **Replace & Continue** -- Creates the new workflow from the template and opens it in the Builder. Your previous workflow is still saved and accessible from the workflow list; it is not deleted.
- **Cancel** -- Closes the dialog and returns you to the gallery without changing anything.

---

## Available Templates

Here is a summary of every template currently available in the gallery.

### Beginner

| Template | Category | Description |
|----------|----------|-------------|
| **API Integration** | Integration | Fetches product data from a public API, transforms the response, and logs the result. A good first workflow to try. |
| **Scheduled Reminder** | Automation | Waits 5 seconds, then calls an API and logs the response. Demonstrates timed delays between steps. |
| **Encode & Hash** | Security | Base64-encodes a string, then SHA-256 hashes it. Shows how to chain data transformation nodes. |
| **Email Notification** | Notification | Simulates processing with a short delay, then sends an email notification. |

### Intermediate

| Template | Category | Description |
|----------|----------|-------------|
| **Data Pipeline** | Data | Fetches a list of users, extracts names with a transform expression, and logs the output. A classic ETL pattern. |
| **Webhook Handler** | Integration | Receives a webhook, delays briefly, then makes an HTTP callback with the result. |
| **AI Text Summarizer** | AI / ML | Fetches an article via HTTP, summarizes it with an AI node, and logs the summary. |
| **Sentiment Analyzer** | AI / ML | Analyzes the sentiment of text input using AI and logs the classification result. |
| **Data Sort & Filter** | Data | Fetches a dataset, filters records by a condition, and logs the filtered output. |

### Advanced

| Template | Category | Description |
|----------|----------|-------------|
| **Parallel Processing** | Data | Fetches data from two APIs simultaneously (fan-out), then collects and logs both results (fan-in). |

---

## Tips

- **Start with a Beginner template** if you are new to Orbflow. The API Integration template runs instantly and shows you the basics of connecting nodes together.
- **Customize after applying** -- Templates are starting points. Once a template is loaded in the Builder, you can add, remove, or reconfigure any node.
- **Use search to find specific nodes** -- If you know you need an HTTP node or an AI node, type that into the search bar. The search checks node names inside templates, not just template titles.
