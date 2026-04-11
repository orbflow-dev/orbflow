# Reviews

The **Reviews** page is where your team collaborates on workflow changes before they go live. Instead of editing a workflow directly, you can propose changes, discuss them with reviewers, and merge only when everyone is satisfied. Think of it as a safety net that keeps your production workflows stable while still allowing your team to iterate quickly.

![Reviews](screenshots/07-reviews.png)

---

## What are change requests?

A change request captures a proposed modification to a workflow. It stores a snapshot of the canvas at the time you create it, along with a title, description, author, and optional reviewers. Anyone on your team can then compare the proposed changes against the current workflow, leave comments, and approve or reject the proposal.

Change requests matter because they give your team:

- **Visibility** -- everyone can see what is being changed and why.
- **Accountability** -- each change has an author and a review trail.
- **Safety** -- modifications only reach the live workflow after explicit approval and merge.
- **Traceability** -- the activity timeline records every action taken on a change request.

---

## Getting started

When you first open the Reviews page without selecting a workflow, you will see an empty state with a pull-request icon and the message:

> **Change Requests**
> Select a workflow from the Builder tab to propose and review changes collaboratively.

To begin, navigate to the **Builder** tab in the sidebar, select (or create) a workflow, then return to the **Reviews** tab. The page will now show the change request list for that workflow.

---

## Browsing change requests

Once a workflow is selected, the Reviews page shows a list of all change requests for that workflow. Each entry displays:

- **Title** -- a short summary of the proposed change.
- **Status badge** -- the current state (Draft, Open, Approved, Rejected, or Merged).
- **Author** -- who created the request, shown with an avatar initial.
- **Comment count** -- how many comments have been left, with unresolved comments highlighted in amber.
- **Base version** -- the workflow version the change was based on.
- **Time** -- a relative timestamp (e.g., "2h ago", "3d ago").

You can filter the list by status using the tab bar at the top (All, Open, Draft, Approved, Rejected, Merged). When you have more than three change requests, a search bar appears so you can find entries by title or author name. Click any entry to open its review view.

---

## Creating a change request

1. Open the **Builder** tab and make your desired changes to the workflow on the canvas.
2. Switch to the **Reviews** tab and click the **New CR** button in the top-right corner.
3. Fill in the creation form:

| Field | Required | Description |
|-------|----------|-------------|
| **Title** | Yes | A concise summary of your changes (max 200 characters). For example, "Add retry logic to HTTP nodes". |
| **Description** | No | A longer explanation of why these changes are needed and what problem they solve (max 5,000 characters). Markdown is supported. |
| **Author** | Yes | Your name (max 100 characters). |
| **Reviewers** | No | A comma-separated list of reviewer names (e.g., "alice, bob, carol"). Each name appears as a tag below the field. |

On the right side of the form, a **preview sidebar** shows:

- **Canvas Snapshot** -- the number of nodes and edges captured from the current canvas state.
- **Readiness checklist** -- three checks (Title filled, Author filled, Canvas captured) with green indicators as each is satisfied.
- A note explaining that the current canvas will be captured as a snapshot for reviewers to compare against the live workflow.

When you are ready, choose one of two actions in the footer:

- **Save Draft** -- saves the change request in Draft status without submitting it for review. You can submit it later.
- **Submit for Review** -- saves and immediately opens the change request for review.

You can also press **Ctrl+Enter** (or **Cmd+Enter** on Mac) to submit quickly, or **Escape** to cancel.

---

## Reviewing a change request

Click any change request from the list to open the review view. The header shows the title, status badge, author, base version, and comment count (with unresolved comments called out in amber).

The review view has three tabs:

### Diff View

The Diff View presents a **side-by-side visual comparison** of the workflow. The left panel shows the **Base Version** (the workflow as it existed when the change request was created) and the right panel shows the **Proposed Changes** (the new version). Both panels render the workflow as an interactive flow graph.

Nodes are color-coded to highlight differences:

| Color | Meaning |
|-------|---------|
| **Green** | Added -- new nodes that do not exist in the base version. |
| **Red** | Removed -- nodes that existed in the base version but are deleted in the proposal. |
| **Cyan** | Modified -- nodes that exist in both versions but have been changed. |
| **Gray** | Unchanged -- nodes that are identical in both versions. |

A **legend bar** at the bottom summarizes the total count of added, removed, and modified nodes. If there are no differences, it shows "No changes detected".

### Comments

The Comments tab is where reviewers discuss the proposed changes. Each comment shows the author, a relative timestamp, and the comment body. Comments can be filtered to show **All** or only **Unresolved** ones.

To leave a comment:

1. Enter your name in the "Your name" field.
2. Type your feedback in the comment box.
3. Click **Comment** or press **Ctrl+Enter** / **Cmd+Enter**.

Any comment can be marked as **Resolved** by clicking the "Resolve" button next to it. Resolved comments appear dimmed so the conversation stays focused on open items. The unresolved count is visible in the review header and in the change request list.

### Activity

The Activity tab shows a chronological timeline of everything that has happened on the change request: creation, submission, comments, approval, rejection, and merge events. Each event displays a color-coded icon, a description, and a relative timestamp. This gives you a complete audit trail at a glance.

---

## Stale base version warning

If the live workflow has been updated since the change request was created, a yellow banner appears below the header:

> Base version is outdated (v1 -> v3). Rebase to update the diff against the latest workflow.

Click the **Rebase** button to update the change request's base version. This refreshes the diff so reviewers can accurately compare against the latest workflow state.

---

## Approval and merge process

The available actions in the review footer depend on the change request's current status:

| Status | Available Actions | What Happens |
|--------|-------------------|-------------|
| **Draft** | Submit for Review | Moves the change request to Open status so reviewers can evaluate it. |
| **Open** | Approve, Reject | Reviewers can approve the changes or reject them. The author is notified. |
| **Approved** | Merge | Applies the proposed definition to the live workflow. A confirmation dialog appears before merging. |
| **Merged** | None | A message confirms the change request has been merged. No further actions are available. |
| **Rejected** | None | A message confirms the change request was rejected. No further actions are available. |

Rejecting a change request also triggers a confirmation dialog to prevent accidental rejections.

---

## Change request lifecycle

Every change request moves through a defined set of states:

**Draft** then **Open** then either **Approved** (which can be **Merged**) or **Rejected**.

- **Draft** -- the change request has been saved but not yet submitted for review. Only the author sees it in their list by default. Use this state to prepare your proposal before it is ready for feedback.
- **Open** -- the change request is submitted and visible to all reviewers. Discussion and review happen in this state.
- **Approved** -- a reviewer has approved the changes. The change request is ready to be merged into the live workflow.
- **Rejected** -- a reviewer has determined the changes should not be applied. The author can create a new change request with revised changes if needed.
- **Merged** -- the proposed changes have been applied to the live workflow. The change request is now part of the workflow's history.

---

## Tips

- **Write clear titles.** A good title like "Add retry logic to HTTP nodes" makes it easy to scan the change request list. Avoid vague titles like "Updates" or "Fix stuff".
- **Use the description field.** Explain the "why" behind your changes so reviewers have context. Markdown formatting helps keep longer explanations readable.
- **Resolve comments as you address them.** This keeps the conversation focused and makes it clear which feedback has been handled.
- **Watch for the stale version banner.** If you see it, rebase before merging to make sure your changes are compatible with the latest workflow.
- **Use filters to stay organized.** The status filter tabs and search bar help you quickly find the change requests that need your attention.
