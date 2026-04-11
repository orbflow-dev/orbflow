# Analytics

The Analytics page gives you a real-time view of how your workflows are performing. It tracks execution counts, success rates, latency percentiles, and per-node breakdowns so you can spot bottlenecks, catch failure trends early, and make informed decisions about where to optimize.

![Analytics](screenshots/03-analytics.png)

You can open the Analytics page from the left sidebar by clicking **Analytics**. The page header reads "Execution metrics & SLA" (Service Level Agreement) to remind you that everything here ties back to your workflow reliability targets.

---

## Selecting a Workflow

A **workflow selector dropdown** sits in the top-right corner of the page. It lists every workflow in your workspace by name.

- When you first open the page, the dropdown automatically selects your first workflow.
- To switch, click the dropdown and choose a different workflow. The dashboard reloads with that workflow's metrics.
- If the currently selected workflow is deleted, the page falls back to the next available workflow.
- If no workflow is selected, you will see a prompt: "Select a workflow -- Choose a workflow above to view execution metrics."

---

## Metrics Overview

Once a workflow is selected and has at least one execution, the top of the page displays four **metric cards** arranged in a grid:

| Card | What it shows |
|------|---------------|
| **Total Executions** | The cumulative number of times this workflow has run, with the start date shown below. |
| **Success Rate** | The percentage of executions that completed without errors. Color-coded: green for 95%+, amber for 80-94%, red below 80%. |
| **Avg Duration** | The average wall-clock time per execution. The P99 duration (the slowest 1% of runs take longer than this) is noted underneath for context. |
| **Failed** | The total count of failed executions. Shows the failure rate as a percentage when failures exist, or "No failures" when everything is healthy. |

Each card includes a small **sparkline** that gives you a visual trend at a glance.

Next to the metric cards is a **success gauge** -- a circular arc that fills based on your success rate. The gauge uses the same color coding (green/amber/red) and displays the exact percentage in the center. This makes it easy to assess overall workflow health in a single glance.

---

## Latency Distribution

Below the overview cards, the **Latency Distribution** section breaks down how long your workflow executions take. Percentiles tell you what fraction of runs finish within a given time -- for example, P95 means "95% of runs are faster than this."

| Percentile | Meaning |
|------------|---------|
| **P50** | The median duration -- half your executions finish faster than this, half take longer. |
| **P95** | 95% of executions finish within this time. A good benchmark for reliability targets. |
| **P99** | The near-worst-case duration -- only 1 in 100 executions takes longer than this. |
| **Avg** | The simple average across all executions. |

Each bar is drawn proportionally against the P99 value so you can visually compare them. Hovering over a bar shows the exact percentage relative to the maximum. A color-coded **summary strip** below the bars repeats the values for quick reference.

A **Refresh** button in the top-right corner of this section lets you reload the latest metrics without leaving the page.

---

## Per-Node Performance

The **Node Performance** table at the bottom of the page lists every node in your workflow along with its individual metrics:

| Column | Description |
|--------|-------------|
| **Node** | The node ID and its type (e.g., `http_request`, `transform`). |
| **Runs** | How many times this specific node has executed. |
| **Success** | The node's individual success rate, shown as a colored badge (green/amber/red). |
| **Avg** | Average execution duration for this node. |
| **P95** | The 95th-percentile duration for this node. |

Click any column header to **sort** the table by that metric. Click the same header again to toggle between ascending and descending order. A small arrow icon indicates the active sort column and direction.

This table is the best place to identify which specific node is causing slowdowns or failures in your workflow.

---

## Empty State

If the selected workflow has never been executed, the page shows an empty state with a chart icon and the message:

> **No executions yet**
> Run this workflow to start collecting execution metrics, latency percentiles, and per-node performance insights.

To populate the analytics dashboard, go back to the **Builder** tab, open the workflow, and run it. Metrics will appear after the first execution completes.

---

## Using Analytics to Improve Your Workflows

Here are practical ways to use the data on this page:

- **Watch the success rate trend.** If it drops below your reliability target (e.g., 95%), investigate the failing nodes in the per-node table to find the root cause.
- **Compare P50 and P95.** A large gap between these two values means most executions are fast, but some are hitting edge cases that cause slowdowns. Look at the slowest nodes to understand why.
- **Sort nodes by P95 duration.** The node with the highest P95 is your bottleneck. Consider whether it can be optimized, cached, or split into smaller steps.
- **Sort nodes by success rate (ascending).** Nodes with the lowest success rates are the most fragile. Check their configuration, upstream dependencies, and error handling.
- **Use the refresh button after changes.** After you modify a workflow and run it a few more times, refresh the metrics to see whether your changes improved performance.
- **Track failed execution counts.** Even a small number of failures can signal a configuration problem, an unreliable external API, or a missing error-handling path in your workflow.
