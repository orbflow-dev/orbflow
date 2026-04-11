# Research Report: Review Prompts Improvement

> Research on Claude Code review prompt best practices and prompt engineering
> to split orbflow-review-prompts.md into frontend and Rust backend files.

## Table of Contents

1. [Automated Post-Implementation Review Workflows](#automated-post-implementation-review-workflows) — 10 principles, 5 anti-patterns
2. [Claude Code Agent Prompt Best Practices](#claude-code-agent-prompt-best-practices) — 10 principles, 6 anti-patterns
3. [Frontend Code Review Prompt Patterns](#frontend-code-review-prompt-patterns) — 10 principles, 6 anti-patterns
4. [Generic Implementation Review Template Design](#generic-implementation-review-template-design) — 10 principles, 6 anti-patterns
5. [Prompt Engineering Anti-Patterns for Code Review](#prompt-engineering-anti-patterns-for-code-review) — 10 principles, 7 anti-patterns
6. [Rust Backend Code Review Prompt Patterns](#rust-backend-code-review-prompt-patterns) — 10 principles, 6 anti-patterns

---

## 1. Automated Post-Implementation Review Workflows

### Key Principles


  - Use Stop hooks to trigger automated review after every implementation wave -- the Stop event fires when Claude finishes responding, making it the natural insertion point for post-implementation checks. Guard against infinite loops by checking the stop_hook_active field.
  - Leverage parallel subagent dispatch for multi-perspective review -- launch 3-5 independent review agents simultaneously (security, performance, CLAUDE.md compliance, bug scanning, test coverage) to get thorough coverage without sequential bottleneck.
  - Pass wave context via git diff and changed file lists -- use git diff --staged, git diff HEAD~1, and git log to automatically derive the set of changed files and pass them as structured context to each review agent.
  - Apply confidence-based scoring (0-100) with a threshold (default 80) to filter false positives -- each review agent scores its findings, and only high-confidence issues surface to the developer.
  - Separate deterministic checks (linting, formatting, type-checking) into PostToolUse command hooks and judgment-based review into prompt/agent hooks -- deterministic rules should never rely on LLM judgment.
  - Use slash commands (.claude/commands/) as the developer-facing trigger for on-demand reviews, and hooks for automatic enforcement -- slash commands are markdown files that define reusable prompts with tool access control.
  - Scope each review agent to its domain to avoid overlap -- agents instructed to only flag issues within the git diff produce higher signal than agents scanning the entire codebase.
  - Aggregate results from parallel agents into a single structured report with deduplication -- the lead agent or a final aggregation step merges findings, removes duplicates, and ranks by severity.
  - Guard Stop hooks against re-triggering loops by parsing the stop_hook_active boolean from stdin JSON and exiting early (exit 0) when true.
  - Store hook configuration in .claude/settings.json for project-specific hooks and ~/.claude/settings.json for global hooks -- project hooks travel with the repo, global hooks apply everywhere.

### Concrete Examples


  - **title:** Stop hook that triggers post-implementation review | **code:** // In .claude/settings.json
{
  "hooks": {
    "Stop": [
      {
        "matcher": "",
        "hooks": [
          {
            "type": "command",
            "command": "bash $CLAUDE_PROJECT_DIR/scripts/post-impl-review.sh"
          }
        ]
      }
    ]
  }
}

// scripts/post-impl-review.sh
#!/bin/bash
INPUT=$(cat)
if [ "$(echo $INPUT | jq -r '.stop_hook_active')" = "true" ]; then
  exit 0  # Prevent infinite loop
fi
CHANGED=$(git diff --name-only HEAD~1)
if [ -z "$CHANGED" ]; then
  exit 0  # No changes, skip review
fi
echo "Run /review-wave on the following changed files: $CHANGED"
exit 2  # Exit code 2 = inject message and continue
  - **title:** Slash command for parallel multi-agent review (/review-wave) | **code:** // .claude/commands/review-wave.md
---
allowed-tools: Bash(git diff:*), Bash(git log:*), Bash(git show:*)
description: Post-implementation review of recent changes
---

Review the most recent implementation wave.

1. Run `git diff HEAD~1 --stat` to identify changed files.
2. Run `git diff HEAD~1` to get the full diff.
3. Launch 3 parallel agents:
   - Agent 1 (Security): Scan diff for security issues (injection, auth bypass, secret leaks)
   - Agent 2 (Architecture): Check for CLAUDE.md compliance, immutability violations, file size limits
   - Agent 3 (Correctness): Look for logic bugs, missing error handling, untested paths
4. Each agent scores findings 0-100 confidence. Only report issues >= 80.
5. Aggregate and deduplicate findings into a single report.
  - **title:** PostToolUse hook for auto-formatting after file edits | **code:** // In .claude/settings.json
{
  "hooks": {
    "PostToolUse": [
      {
        "matcher": "(Edit|Write)",
        "hooks": [
          {
            "type": "command",
            "command": "FILE=$(cat | jq -r '.tool_input.file_path // empty'); if [[ $FILE == *.ts || $FILE == *.tsx ]]; then npx prettier --write \"$FILE\"; fi"
          }
        ]
      }
    ]
  }
}
  - **title:** Agent team for comprehensive review with role specialization | **code:** // Prompt to Claude Code lead session:
"Create an agent team to review the changes from this implementation wave.
Spawn 3 teammates:
  1. security-reviewer: Check all changed files for security issues. Focus on input validation, auth, and secret handling.
  2. perf-reviewer: Check for N+1 queries, unnecessary allocations, missing indexes.
  3. test-reviewer: Verify test coverage for changed code paths. Flag untested branches.
Each teammate should use `git diff main...HEAD` to scope their review.
Require plan approval before teammates begin.
Aggregate all findings into a ranked list when done."
  - **title:** Hook input JSON structure for context passing | **code:** // What a Stop hook receives on stdin:
{
  "session_id": "abc123",
  "cwd": "/Users/dev/orbflow-rust",
  "hook_event_name": "Stop",
  "stop_hook_active": false,
  "transcript_summary": "Implemented HTTP retry logic in orbflow-builtins..."
}

// Script extracts context and passes to review:
#!/bin/bash
INPUT=$(cat)
ACTIVE=$(echo $INPUT | jq -r '.stop_hook_active')
if [ "$ACTIVE" = "true" ]; then exit 0; fi
CWD=$(echo $INPUT | jq -r '.cwd')
DIFF=$(cd $CWD && git diff --name-only HEAD~1 2>/dev/null)
if [ -n "$DIFF" ]; then
  echo "Please review these changed files for issues: $DIFF"
  exit 2
fi
exit 0

### Anti Patterns


  - **pattern:** Stop hook without loop guard | **why_it_fails:** A Stop hook that always returns exit code 2 (continue) creates an infinite loop -- Claude finishes the review, the Stop hook fires again, triggers another review, and so on forever. Always check stop_hook_active and exit 0 when true. | **instead:** Parse the stop_hook_active field from stdin JSON. If true, exit 0 immediately. Only trigger the review on the first Stop event after implementation.
  - **pattern:** Single monolithic review agent | **why_it_fails:** A single agent reviewing everything (security + performance + correctness + style) gravitates toward one type of issue and misses others. It also takes longer and uses more context window. | **instead:** Launch 3-5 specialized parallel agents, each focused on one review domain. Aggregate results afterward. This mirrors the official Code Review Plugin pattern.
  - **pattern:** Review agents scanning the entire codebase instead of the diff | **why_it_fails:** Reviewing unchanged code wastes tokens, produces irrelevant findings, and overwhelms the developer with noise. Agents lose focus and produce lower-quality findings. | **instead:** Scope each agent to only the git diff output. Instruct agents explicitly: 'Only flag issues that exist within the changed code shown in the diff.'
  - **pattern:** Relying on LLM judgment for deterministic checks | **why_it_fails:** Formatting, linting, and type-checking are deterministic -- asking an LLM to check them is unreliable and wasteful. The LLM may miss obvious violations or hallucinate false ones. | **instead:** Use PostToolUse command hooks for deterministic checks (prettier, clippy, eslint). Reserve prompt/agent hooks for judgment-based review (architecture, security reasoning, design patterns).
  - **pattern:** No confidence scoring or filtering on review output | **why_it_fails:** Without scoring, review agents produce many low-confidence findings that are often false positives. Developers learn to ignore review output entirely, defeating the purpose. | **instead:** Require each agent to score every finding 0-100. Apply a threshold (80+ recommended). Only surface high-confidence issues. This is the pattern used by Anthropic's official Code Review Plugin.

### Implementation Recommendations


  - Create a .claude/commands/review-wave.md slash command that runs post-implementation review scoped to recent git changes. This gives developers an on-demand trigger while hooks provide automatic enforcement.
  - For orbflow's frontend (apps/web, packages/orbflow-core): create a frontend-specific review agent that checks for React anti-patterns, Zustand store immutability, CSS theme consistency, and component file size limits (<800 lines per CLAUDE.md guidelines).
  - For orbflow's Rust backend (crates/*): create a backend-specific review agent that checks for port trait compliance, error handling via OrbflowError, proper use of NodeExecutor pattern, and that dependencies point inward (only orbflow-core imported across crate boundaries).
  - Configure a PostToolUse hook on Edit/Write tools to auto-run prettier on .ts/.tsx files and cargo fmt on .rs files, ensuring style consistency without LLM involvement.
  - Use a Stop hook that detects whether the session involved code changes (via git diff) and automatically triggers /review-wave. Include the stop_hook_active guard to prevent loops.
  - Store review agent definitions in .claude/agents/ (e.g., code-reviewer.md, security-reviewer.md) following the existing project convention, and reference them from the slash command.
  - For the monorepo structure, have the review slash command detect which workspace was modified (apps/web vs packages/orbflow-core vs crates/) and route to the appropriate specialized reviewer.

### Prompt Template Fragments


  - ## Post-Implementation Review Instructions

You are reviewing code changes from the most recent implementation wave. Your review MUST be scoped to the git diff only -- do not flag issues in unchanged code.

### Context Gathering
1. Run `git diff HEAD~1 --stat` to identify changed files
2. Run `git diff HEAD~1` to get the full diff
3. Identify which workspace was modified: Rust backend (crates/), frontend app (apps/web/), or shared package (packages/orbflow-core/)

### Review Scope
- Only flag issues visible in the diff
- Score each finding 0-100 confidence
- Only report findings with confidence >= 80
- Categorize: CRITICAL (must fix), HIGH (should fix), MEDIUM (consider fixing)
  - ## Parallel Agent Dispatch Template

Launch the following agents in parallel:

### Agent 1: Security Review
Scan the diff for: hardcoded secrets, SQL injection, XSS, CSRF, missing auth checks, error messages leaking internal details. Score each finding 0-100.

### Agent 2: Architecture Review  
Check for: CLAUDE.md compliance, immutability violations (mutations instead of new copies), file size > 800 lines, functions > 50 lines, deep nesting > 4 levels, missing error handling, hardcoded values.

### Agent 3: Correctness Review
Look for: logic bugs, missing edge cases, untested code paths, incorrect error variants, wire format mismatches between frontend and backend.
  - ## Result Aggregation Template

After all review agents complete:
1. Collect all findings from each agent
2. Deduplicate findings that flag the same code location
3. Sort by: CRITICAL first, then HIGH, then MEDIUM
4. For each finding, include:
   - File path and line range
   - Category (security/architecture/correctness)
   - Description of the issue
   - Suggested fix
   - Confidence score
5. Present as a structured report
  - ## Stop Hook Review Guard

```bash
#!/bin/bash
# scripts/review-guard.sh - Triggers review after implementation, prevents loops
INPUT=$(cat)
ACTIVE=$(echo $INPUT | jq -r '.stop_hook_active // false')
if [ "$ACTIVE" = "true" ]; then
  exit 0
fi
CHANGED=$(git diff --name-only HEAD~1 2>/dev/null | head -50)
if [ -z "$CHANGED" ]; then
  exit 0
fi
echo "Implementation wave complete. Please run /review-wave to review: $CHANGED"
exit 2
```
  - ## Orbflow-Specific Review Checklist

### Rust Backend
- [ ] OrbflowError variants used correctly (not generic errors)
- [ ] NodeExecutor pattern followed for new builtins
- [ ] Dependencies point inward (only orbflow-core imported across crates)
- [ ] Immutable domain objects (new copies, not mutations)
- [ ] Wire types use snake_case matching frontend api.ts

### Frontend
- [ ] Zustand stores use immutable update patterns
- [ ] Components < 800 lines, functions < 50 lines
- [ ] Theme system used (no hardcoded colors)
- [ ] CEL expressions properly handled (= prefix convention)
- [ ] API client types match backend wire format

### Sources


  - https://code.claude.com/docs/en/hooks-guide - Official Claude Code Hooks documentation with event types, input/output schemas, and configuration
  - https://code.claude.com/docs/en/agent-teams - Official Claude Code Agent Teams documentation for parallel agent orchestration
  - https://github.com/anthropics/claude-code/blob/main/plugins/code-review/README.md - Anthropic's official Code Review Plugin with parallel agents and confidence scoring
  - https://github.com/anthropics/claude-code/blob/main/plugins/code-review/commands/code-review.md - Code Review Plugin slash command implementation showing 4-agent parallel review
  - https://dev.to/bredmond1019/multi-agent-orchestration-running-10-claude-instances-in-parallel-part-3-29da - Multi-agent orchestration patterns for Claude Code
  - https://github.com/hesreallyhim/awesome-claude-code - Curated list of Claude Code hooks, slash commands, and agent orchestrators
  - https://blog.gitbutler.com/automate-your-ai-workflows-with-claude-code-hooks - Practical guide to Claude Code hooks automation
  - https://claudefa.st/blog/guide/agents/sub-agent-best-practices - Sub-agent parallel vs sequential execution patterns
  - https://github.com/affaan-m/everything-claude-code/blob/main/agents/code-reviewer.md - Community code reviewer agent example
  - https://www.eesel.ai/blog/hooks-in-claude-code - Practical guide to Claude Code hooks with examples (2026)

### Relevance To Orbflow


  > Orbflow is a distributed workflow automation engine with a Rust backend (ports-and-adapters architecture across 15+ crates) and a TypeScript/React frontend monorepo. Post-implementation review workflows are directly relevant because orbflow's architecture enforces strict conventions that are easy to violate: dependencies must point inward through orbflow-core, domain objects must be immutable, NodeExecutor implementations must follow a specific pattern, and wire types must use snake_case matching the frontend api.ts types. Automated review workflows using Claude Code hooks and parallel agents can enforce these conventions after every implementation wave without manual oversight.

The split between frontend and backend makes multi-agent parallel review particularly valuable -- a Rust-focused agent can check port trait compliance, error handling via OrbflowError, and crate boundary rules, while a frontend-focused agent can verify Zustand store immutability, component file sizes, theme system usage, and CEL expression handling. The monorepo structure (apps/web, packages/orbflow-core, crates/) provides natural workspace boundaries for routing changes to the appropriate specialized reviewer.

### Automation Hooks


  - Stop hook with stop_hook_active guard: fires after Claude finishes responding, checks git diff for changes, and injects a /review-wave prompt if changes exist. Uses exit code 2 to continue the session with the review prompt.
  - PostToolUse hook on Edit/Write tools: auto-runs prettier on .ts/.tsx files and cargo fmt on .rs files after every file edit, ensuring deterministic style compliance without LLM involvement.
  - PreToolUse hook on Bash tool: blocks dangerous commands (e.g., git push --force, DROP TABLE) before execution, providing a safety net during implementation waves.
  - Slash command .claude/commands/review-wave.md: developer-facing trigger that runs git diff, identifies changed workspaces, and dispatches parallel review agents scoped to the diff.
  - Agent team orchestration: lead session spawns 3-5 specialized teammates (security-reviewer, arch-reviewer, test-reviewer) each working in their own context window, with results aggregated by the lead.
  - SessionStart hook: on session resume, automatically loads the last wave's review findings from a local file so the developer has context on outstanding issues.
  - Notification hook: sends desktop notification when review agents complete, so the developer can switch tasks during the parallel review phase.
  - Git diff integration: all review agents receive context via git diff HEAD~1 (for post-commit review) or git diff --staged (for pre-commit review), scoping review to only changed code.

---

## 2. Claude Code Agent Prompt Best Practices

### Key Principles


  - Use <investigate_before_answering> tags to force agents to read files before answering: 'Never speculate about code you have not opened. If the user references a specific file, you MUST read the file before answering.' This is Anthropic's own recommended anti-hallucination pattern.
  - Restrict subagent tools via the 'tools' frontmatter field to create hard constraints. A reviewer with only Read, Grep, Glob physically cannot write files -- this is structural, not advisory. The Explore built-in agent uses '=== CRITICAL: READ-ONLY MODE ===' with an explicit deny list.
  - Scope reviews to changed files using git diff. The official code-review plugin runs 'gh pr diff' and instructs agents: 'Focus only on the diff itself without reading extra context. Flag only significant bugs; ignore nitpicks and likely false positives.'
  - Launch parallel specialized subagents rather than one monolithic reviewer. The official code-review plugin launches 4 parallel agents: 2 for CLAUDE.md compliance, 1 for bug scanning (diff-only), 1 for context-based analysis (git blame/history).
  - Apply confidence scoring to filter false positives. Each issue gets a 0-100 confidence score; only issues >= 80 are reported. This dramatically reduces noise in automated reviews.
  - Use a validation pass: after initial review agents find issues, launch additional parallel subagents to validate each issue independently. Only issues confirmed with high confidence survive.
  - Keep CLAUDE.md concise and actionable -- if Claude already does something correctly without the instruction, delete it. Long CLAUDE.md files cause instruction loss. Convert enforceable rules into hooks instead.
  - Pass all necessary context in the subagent prompt string directly. The only channel from parent to subagent is the Agent tool's prompt -- file paths, error messages, decisions, and constraints must be included explicitly.
  - Use hooks (PreToolUse/PostToolUse) for deterministic enforcement rather than relying on prompt instructions. Hooks guarantee behavior (e.g., auto-format after edits, block writes to protected files) where prompts only suggest it.
  - Separate concerns with instruction hierarchy: CLAUDE.md for always-on project context, skills for on-demand domain knowledge, subagents for isolated context windows, and hooks for deterministic guardrails.

### Concrete Examples


  - **title:** Anti-hallucination wrapper for code analysis | **before:** Review the authentication module for security issues. | **after:** <investigate_before_answering>
Never speculate about code you have not opened. If the user references a specific file, you MUST read the file before answering. Make sure to investigate and read relevant files BEFORE answering questions about the codebase. Never make any claims about code before investigating unless you are certain of the correct answer.
</investigate_before_answering>

Review the authentication module in crates/orbflow-httpapi/src/middleware.rs for security issues. Read the file first, then analyze. | **explanation:** The XML-tagged instruction block forces the agent to use Read/Grep tools before making claims. Specifying the exact file path prevents hallucinated paths.
  - **title:** Read-only subagent with tool restrictions | **before:** ---
name: code-reviewer
description: Reviews code changes
---
You are a code reviewer. Review the changes and suggest fixes. | **after:** ---
name: code-reviewer
description: Reviews code for quality, security, and best practices. Use proactively after code changes.
tools: Read, Grep, Glob
model: sonnet
---

You are an expert code reviewer. You have READ-ONLY access.

=== CRITICAL: NO FILE MODIFICATIONS ===
You are STRICTLY PROHIBITED from creating, modifying, or deleting files.

Workflow:
1. Run `git diff --name-only HEAD~1` to identify changed files
2. Read each changed file using the Read tool
3. Search for related patterns using Grep
4. Analyze and report findings

For each issue provide:
- Severity: CRITICAL / HIGH / MEDIUM / LOW
- File path and line range
- Description with code snippet
- Suggested fix (as text, do not edit)
- Confidence score (0-100, only report >= 80) | **explanation:** The 'tools' field structurally prevents writes. The explicit workflow forces file reads before analysis. Confidence scoring filters noise.
  - **title:** Scoping review to changed files only | **before:** Review the codebase for issues. | **after:** Review ONLY the files changed in this PR. Use `gh pr diff $PR_NUMBER` to get the diff.

Do NOT read files outside the diff unless needed to understand an import or type reference.
Do NOT flag issues in unchanged code.
Do NOT flag issues that you cannot validate without looking at context outside of the git diff.

For each issue, cite the exact file path and line number from the diff. | **explanation:** Explicit scoping prevents the agent from reading hundreds of files and filling context. Requiring exact citations prevents hallucinated references.
  - **title:** Parallel agent orchestration for code review | **before:** Review this PR thoroughly. | **after:** Follow these steps precisely:

1. Launch a haiku agent to check if the PR is closed, draft, or already reviewed. If so, stop.

2. Launch a haiku agent to list file paths of all relevant CLAUDE.md files in directories containing modified files.

3. Launch a sonnet agent to summarize the PR changes.

4. Launch 4 agents in parallel:
   - Agents 1+2 (sonnet): Audit CLAUDE.md compliance
   - Agent 3 (opus): Scan for bugs in diff only, no extra context
   - Agent 4 (opus): Analyze with git blame history context

5. For each issue from step 4, launch validation subagents in parallel. Each validator reviews the issue independently and assigns confidence 0-100.

6. Filter to issues with confidence >= 80. Output remaining issues. | **explanation:** This mirrors Anthropic's official code-review plugin pattern. Parallel agents preserve context, validation passes filter false positives, and model selection optimizes cost.
  - **title:** Hook-based enforcement vs prompt-based suggestion | **before:** Always run prettier after editing files (instruction in CLAUDE.md) | **after:** {
  "hooks": {
    "PostToolUse": [
      {
        "matcher": "Edit|Write",
        "hooks": [
          {
            "type": "command",
            "command": "npx prettier --write \"$TOOL_INPUT_FILE_PATH\""
          }
        ]
      }
    ]
  }
} | **explanation:** Hooks provide deterministic guarantees. The CLAUDE.md instruction can be forgotten after context compaction; the hook always fires. Use hooks for rules, prompts for judgment.

### Anti Patterns


  - **pattern:** Vague scope: 'Review the codebase' without specifying files or diff | **why_it_fails:** The agent reads hundreds of files, fills the context window, and produces unfocused results. Earlier instructions get lost after compaction. The agent may hallucinate issues in files it never actually read. | **alternative:** Always scope to changed files: 'Review only files in `git diff --name-only main...HEAD`'. Pass explicit file paths in the prompt when possible.
  - **pattern:** Relying on the agent to 'know' file paths or function signatures from training data | **why_it_fails:** Claude may hallucinate file paths, function names, or API signatures that look plausible but do not exist in the actual codebase. This is especially common in subagents which have no prior conversation context. | **alternative:** Always instruct the agent to use Glob/Grep/Read to discover and verify paths before referencing them. Use the <investigate_before_answering> pattern. Pass known paths explicitly in the prompt string.
  - **pattern:** Putting all enforcement rules in CLAUDE.md instead of using hooks | **why_it_fails:** CLAUDE.md is loaded once and can be forgotten after context compaction. Rules like 'always run linter' or 'never edit config files' are advisory, not enforced. The agent may comply 90% of the time but fail on the critical 10%. | **alternative:** Use PreToolUse hooks to block forbidden operations (exit code 2) and PostToolUse hooks for mandatory post-actions (formatting, linting). Reserve CLAUDE.md for context that requires judgment.
  - **pattern:** Single monolithic review agent trying to check everything | **why_it_fails:** One agent checking security, style, bugs, tests, and CLAUDE.md compliance in a single pass produces shallow results. The context fills up with code reads, leaving little room for analysis. Different concerns benefit from different models (opus for bugs, sonnet for compliance). | **alternative:** Launch parallel specialized subagents, each with a focused mandate and appropriate model. Merge and deduplicate results afterward.
  - **pattern:** Not validating agent-found issues before reporting | **why_it_fails:** Initial review passes produce many false positives, especially for style and 'potential bug' categories. Reporting all of them erodes trust in the review system and creates alert fatigue. | **alternative:** Add a validation pass where independent subagents review each issue. Apply confidence scoring (0-100) and only surface issues >= 80 confidence. The official Claude Code review plugin uses this exact pattern.
  - **pattern:** Overloading CLAUDE.md with instructions the model already follows | **why_it_fails:** Long CLAUDE.md files cause important rules to get lost. Every line competes for attention in the context window. If Claude already follows a practice by default, the instruction adds noise without value. | **alternative:** Ruthlessly prune CLAUDE.md. Test whether Claude follows a rule without the instruction. If yes, remove it. Convert deterministic rules to hooks.

### Implementation Recommendations


  - Create separate review subagents for frontend (apps/web, packages/orbflow-core) and backend (crates/*) with domain-specific prompts. The frontend reviewer should check React patterns, Zustand store immutability, and CSS consistency. The backend reviewer should check Rust error handling, port/adapter boundaries, and NodeExecutor conventions.
  - Use git diff to scope reviews: run `git diff --name-only main...HEAD` first, then partition files into frontend (.tsx, .ts in apps/ or packages/) and backend (.rs in crates/) groups. Pass only the relevant file list to each specialized subagent.
  - Implement the <investigate_before_answering> pattern in all review subagents to prevent hallucination of orbflow-specific types like OrbflowError variants, port trait methods, or NodeSchema field definitions. Force agents to Read the actual source before claiming something exists.
  - Apply the official code-review plugin's confidence scoring pattern: each issue gets a 0-100 score, only issues >= 80 are reported. This is critical for orbflow where false positives in complex DAG/CEL code would erode trust.
  - Create a PostToolUse hook that runs `cargo clippy` after Rust file edits and `pnpm lint` after TypeScript file edits to catch issues deterministically rather than relying on prompt instructions.
  - Pass orbflow architectural constraints explicitly in subagent prompts: 'Dependencies point inward -- only orbflow-core is imported across crate boundaries. Flag any import that violates this rule.' Do not assume the agent will infer this from CLAUDE.md.
  - Use haiku-tier agents for triage tasks (file listing, draft detection, CLAUDE.md gathering) and opus-tier agents for deep bug analysis. This optimizes cost while maintaining quality on the tasks that matter most.

### Prompt Template Fragments


  - ## Anti-Hallucination Guard
<investigate_before_answering>
Never speculate about code you have not opened. You MUST read a file using the Read tool before making any claims about its contents, structure, or behavior. Never hallucinate file paths -- use Glob to discover files if you are unsure of their location. Never hallucinate function signatures -- use Grep to find the actual definition.
</investigate_before_answering>
  - ## Scope Restriction
You are reviewing ONLY the files changed in this diff. Run `git diff --name-only $BASE_BRANCH...HEAD` to get the list of changed files.
- Do NOT read files outside this list unless resolving an import or type reference
- Do NOT flag issues in unchanged code
- Do NOT flag issues you cannot validate from the diff context alone
- For each issue, cite the exact file path and line number
  - ## Issue Reporting Format
For each issue found, report:
- **Severity**: CRITICAL | HIGH | MEDIUM | LOW
- **Category**: bug | security | style | architecture | performance
- **File**: exact file path
- **Lines**: line range (e.g., 42-47)
- **Description**: what is wrong and why
- **Evidence**: quote the relevant code
- **Suggestion**: how to fix (as text, do not modify the file)
- **Confidence**: 0-100 (only report issues with confidence >= 80)
  - ## Orbflow Architecture Rules (pass to backend review agents)
This codebase follows Ports & Adapters architecture:
- orbflow-core defines all domain types and port traits
- Every other crate implements exactly one adapter
- Dependencies point INWARD: only orbflow-core may be imported across crate boundaries
- OrbflowError enum in orbflow-core::error is the single error type
- All builtin executors implement NodeExecutor and NodeSchemaProvider traits
- Wire types use snake_case JSON field names matching frontend api.ts

Flag any violation of these rules as CRITICAL.
  - ## Subagent Frontmatter Template (read-only reviewer)
---
name: orbflow-reviewer
description: Reviews orbflow code changes for bugs, architecture violations, and best practices. Use after code modifications.
tools: Read, Grep, Glob, Bash(git diff:*), Bash(git log:*), Bash(git blame:*)
model: sonnet
---

### Sources


  - https://code.claude.com/docs/en/sub-agents - Official Claude Code subagents documentation with frontmatter fields, tool restrictions, and example configurations
  - https://code.claude.com/docs/en/best-practices - Official Claude Code best practices including CLAUDE.md writing guidelines
  - https://platform.claude.com/docs/en/build-with-claude/prompt-engineering/claude-prompting-best-practices - Anthropic's prompting best practices with <investigate_before_answering> anti-hallucination pattern
  - https://github.com/anthropics/claude-code/blob/main/plugins/code-review/commands/code-review.md - Official code-review plugin with parallel agent orchestration, confidence scoring, and validation passes
  - https://github.com/Piebald-AI/claude-code-system-prompts - Reverse-engineered Claude Code system prompts including Explore agent's read-only enforcement pattern
  - https://code.claude.com/docs/en/hooks-guide - Official hooks documentation for PreToolUse/PostToolUse deterministic enforcement
  - https://alexop.dev/posts/claude-code-customization-guide-claudemd-skills-subagents/ - Comprehensive guide comparing CLAUDE.md, slash commands, skills, and subagents with trade-off analysis
  - https://hamy.xyz/blog/2026-02_code-reviews-claude-subagents - Real-world implementation of 9 parallel code review subagents
  - https://towardsdatascience.com/claude-skills-and-subagents-escaping-the-prompt-engineering-hamster-wheel/ - Analysis of skills and subagents for reliable agent behavior
  - https://www.builder.io/blog/claude-code - Practical tips including CLAUDE.md pruning and scoped investigations

### Relevance To Orbflow


  > Orbflow's architecture -- a Rust backend with Ports & Adapters pattern plus a TypeScript/React frontend monorepo -- creates a natural split for review agents. Backend reviews must enforce orbflow-specific invariants: inward-pointing dependencies (only orbflow-core crosses crate boundaries), OrbflowError as the single error type, NodeExecutor/NodeSchemaProvider trait compliance, snake_case wire format consistency with the frontend's api.ts types, and CEL expression safety. Frontend reviews must check Zustand store immutability patterns, React component conventions in the core/ vs components/ layering, and theme consistency.

The parallel subagent pattern from Anthropic's official code-review plugin maps perfectly to orbflow's dual-stack: launch separate frontend and backend reviewer subagents in parallel, each with domain-specific prompts that include orbflow's architectural rules. The confidence scoring pattern (>= 80 threshold) is especially important for orbflow because the DAG engine, CEL evaluator, and event sourcing code involve complex logic where false positive 'bugs' would create noise. Using hooks for deterministic enforcement (cargo clippy after .rs edits, pnpm lint after .ts edits) complements prompt-based review by catching mechanical issues that do not require judgment.

### Automation Hooks


  - PreToolUse hook on Edit|Write matcher to block modifications to protected files (e.g., migration SQL files, proto definitions, Cargo.toml workspace config) during automated review -- exit code 2 with stderr message explaining why the edit is blocked.
  - PostToolUse hook on Edit|Write matcher to auto-run `cargo clippy --workspace -- -D warnings` after any .rs file edit and `pnpm lint` after any .ts/.tsx file edit, ensuring mechanical quality without relying on the agent remembering to lint.
  - PreToolUse hook on Bash matcher to block destructive git operations (push --force, reset --hard, clean -f) during review subagent execution, preventing accidental repository damage.
  - Use `git diff --name-only main...HEAD` in the review skill/command to dynamically generate the file list, then partition into frontend/backend groups and pass each group to the appropriate specialized subagent.
  - PostToolUse hook with 'Notification' event type to send a desktop notification when the review is complete, allowing developers to context-switch while the multi-agent review runs.
  - Implement a slash command `/review` that orchestrates the full review pipeline: (1) haiku agent for triage, (2) parallel sonnet agents for compliance, (3) parallel opus agents for bug detection, (4) parallel validation agents for confidence scoring, (5) formatted output with only high-confidence issues.
  - Use the Agent SDK's --agents flag to pass review subagent definitions as JSON in CI/CD pipelines, enabling headless code review on every PR without requiring .claude/agents/ files in the repository.

---

## 3. Frontend Code Review Prompt Patterns

### Key Principles


  - Use the PCEI framework (Persona, Context, Examples, Instructions) for every review prompt -- assign a specific frontend expert persona, provide tech stack context (React 19, Next.js 15, TypeScript strict, Zustand), show example findings, and give explicit instructions on output format and severity levels.
  - Scope prompts narrowly by review domain rather than using one mega-prompt: separate prompts for component architecture, hooks correctness, render performance, type safety, accessibility, security, and bundle size produce more reliable and thorough findings than a single generic 'review this code' prompt.
  - Always include project-specific architectural constraints in the prompt context: state management library (Zustand), styling approach (Tailwind/CSS modules), component boundaries (server vs client components), and naming conventions. Without this, the LLM invents conventions.
  - Require structured output with severity levels (CRITICAL/HIGH/MEDIUM/LOW), file locations, and concrete fix suggestions. Unstructured prose reviews are harder to action and track.
  - Focus Zustand review prompts on selector specificity: flag components that subscribe to the entire store instead of specific slices, missing useShallow for multi-property selectors, and object creation inside selectors that defeats reference equality.
  - Enforce 'use client' boundary minimization in Next.js App Router reviews: check that interactive components are small client islands imported into server components, not large subtrees unnecessarily marked as client components that bloat the JS bundle.
  - Mandate hooks correctness checks: dependency arrays must include all referenced values, hooks must never appear inside conditionals or loops, custom hooks must follow the use* naming convention, and useEffect cleanup functions must be present for subscriptions and timers.
  - Include frontend-specific security checks as a dedicated prompt section: flag all dangerouslySetInnerHTML usage without DOMPurify sanitization, href='javascript:' patterns, eval() calls, unsanitized URL parameters rendered in the DOM, and postMessage without origin validation.
  - Provide few-shot examples of the exact issue format you want the reviewer to produce -- this is the single most effective technique for improving LLM review output quality. Show 3-5 examples of real findings with the severity, location, explanation, and fix.
  - Review prompts should explicitly list what NOT to flag (e.g., minor style preferences already handled by ESLint/Prettier) to reduce noise and focus the LLM on substantive issues that automated tooling cannot catch.

### Concrete Examples


  - **name:** Generic vs Specific Prompt Comparison | **before:** Review this React code for issues. | **after:** You are a senior React performance engineer reviewing a Next.js 15 App Router application using Zustand for state management and Tailwind CSS for styling. Analyze the following diff for: (1) unnecessary re-renders caused by non-memoized callbacks or object literals passed as props, (2) Zustand selectors that subscribe to more state than needed, (3) missing React.memo on pure presentational components receiving complex props, (4) useEffect hooks with incorrect or missing dependency arrays. For each issue found, provide: severity (CRITICAL/HIGH/MEDIUM/LOW), file and line, explanation, and a concrete code fix. | **why:** The specific prompt assigns a persona, provides stack context, enumerates exact check categories, and defines output format -- producing focused, actionable findings instead of vague suggestions.
  - **name:** Zustand Selector Review Prompt | **prompt:** Review all Zustand useStore() calls in the changed files. Flag any component that: (a) destructures the entire store state instead of selecting specific fields, (b) creates new object references inside the selector function without useShallow, (c) selects derived data that should be computed via a separate derived selector or useMemo. For each finding, show the current code and the optimized selector pattern. | **why:** Zustand-specific prompts catch the most common performance pitfall: subscribing to the entire store causes every state change to re-render the component.
  - **name:** Next.js Client/Server Boundary Review | **prompt:** Audit all files with 'use client' directives. For each client component, check: (1) Could this component be a server component if interactivity were extracted to a smaller child? (2) Does it import heavy libraries (date-fns, lodash, chart libraries) that could stay server-side? (3) Are there data-fetching calls that should use server actions or RSC instead of client-side fetch? List each file with its client-side JS cost assessment. | **why:** Prevents bundle bloat by ensuring the 'use client' boundary is drawn as narrowly as possible.
  - **name:** Security-Focused Frontend Review Prompt | **prompt:** You are a frontend security specialist. Scan these React components for: (1) Any use of dangerouslySetInnerHTML -- if found, verify DOMPurify.sanitize() wraps the input. (2) Dynamic href or src attributes constructed from user input or URL params without validation. (3) Any eval(), new Function(), or innerHTML assignments. (4) postMessage/addEventListener('message') handlers without origin checks. (5) Sensitive data (tokens, keys) stored in localStorage instead of httpOnly cookies. Rate each finding as CRITICAL (exploitable XSS/injection) or HIGH (potential vector). | **why:** React auto-escapes JSX text but has well-known escape hatches. A targeted prompt catches these specific vectors rather than generic 'check for security issues'.
  - **name:** Accessibility Review Prompt Fragment | **prompt:** Review the JSX output for WCAG 2.1 AA compliance: (1) All interactive elements (buttons, links, inputs) must have accessible names via visible text, aria-label, or aria-labelledby. (2) Images must have meaningful alt text (not 'image' or empty string on informational images). (3) Custom components acting as buttons must use <button> or role='button' with keyboard handlers (onKeyDown for Enter/Space). (4) Color contrast: flag any hardcoded color values that may not meet 4.5:1 ratio. (5) Focus management: modals and drawers must trap focus and restore it on close. | **why:** Generic 'check accessibility' prompts miss most issues. Enumerating specific WCAG criteria produces concrete, verifiable findings.

### Anti Patterns


  - **pattern:** The 'review everything' mega-prompt | **why_it_fails:** A single prompt asking the LLM to check architecture, performance, security, accessibility, types, and styling simultaneously produces shallow coverage of each area. The model distributes attention across too many concerns and misses deep issues in each category. | **instead:** Use separate, focused review passes: one for architecture/component structure, one for hooks/render performance, one for type safety, one for security, one for accessibility. Chain them or run in parallel with specialized agent sub-tasks.
  - **pattern:** No project context in the prompt | **why_it_fails:** Without knowing the tech stack, state management approach, and project conventions, the LLM defaults to generic React advice, suggests Redux patterns for a Zustand project, or recommends class component patterns for a hooks-based codebase. It may also 'approve' code that violates project-specific strict TypeScript settings. | **instead:** Always include a context block specifying: React version, Next.js App Router vs Pages Router, state management library, CSS approach, TypeScript strict mode enabled, and any custom architectural rules from CLAUDE.md or REVIEW.md.
  - **pattern:** Reviewing code without the diff context | **why_it_fails:** Reviewing an entire file when only 5 lines changed wastes token budget and produces findings about pre-existing code that is not part of the current change. The reviewer cannot distinguish new issues from existing tech debt. | **instead:** Pass the git diff as input, with enough surrounding context (10-20 lines) for the LLM to understand the change. Explicitly instruct: 'Focus findings on the changed lines. Only flag pre-existing issues if they are CRITICAL security or correctness problems.'
  - **pattern:** Not specifying output format | **why_it_fails:** Without a structured format, the LLM returns prose paragraphs mixing critical bugs with trivial style nits. This makes it hard to prioritize, track, or automate follow-up actions. | **instead:** Define a clear schema: severity level, category (performance/security/correctness/style), file path, line number or code snippet, explanation, and suggested fix. Use a table or JSON format for machine-parseable output.
  - **pattern:** Ignoring what linters already catch | **why_it_fails:** Prompting the LLM to flag unused variables, missing semicolons, or import ordering duplicates work that ESLint and Prettier already handle automatically. This wastes tokens and dilutes the signal-to-noise ratio of the review. | **instead:** Explicitly exclude lint-catchable issues: 'Do not flag formatting, unused imports, or style issues handled by ESLint. Focus on semantic correctness, architecture decisions, and issues requiring human judgment.'
  - **pattern:** Treating all findings as equal severity | **why_it_fails:** Without severity classification, a missing aria-label gets the same weight as an XSS vulnerability via dangerouslySetInnerHTML. Teams cannot triage effectively. | **instead:** Require severity levels with clear definitions: CRITICAL (security vulnerability, data loss, crash), HIGH (correctness bug, major performance regression), MEDIUM (maintainability, minor perf), LOW (suggestions, alternative approaches).

### Implementation Recommendations


  - Create a dedicated frontend review prompt file (e.g., REVIEW-FRONTEND.md) separate from the backend review prompt. Frontend reviews have fundamentally different concerns (DOM rendering, CSS, bundle size, browser APIs) that require specialized instructions.
  - Structure the frontend review prompt into clearly labeled sections with XML tags: <component-architecture>, <hooks-correctness>, <render-performance>, <type-safety>, <zustand-state>, <nextjs-patterns>, <accessibility>, <security>, <bundle-size>. This lets reviewers selectively enable/disable sections based on what changed.
  - For orbflow's monorepo structure (apps/web + packages/orbflow-core), include a section mapping which review checks apply to which workspace: orbflow-core (headless SDK) should emphasize type safety, API contracts, and store patterns; apps/web should additionally cover JSX accessibility, CSS/Tailwind usage, and Next.js App Router patterns.
  - Embed orbflow-specific Zustand patterns in the prompt: orbflow uses canvasStore, workflowStore, executionOverlayStore, and credentialStore. The review prompt should explicitly check that components select only needed slices from these stores and use useShallow when destructuring multiple properties.
  - Include a orbflow-specific 'use client' boundary audit section since apps/web uses Next.js 15 App Router. The prompt should verify that src/core/ components (embeddable builder) correctly manage client/server boundaries and that heavy dependencies are not pulled into client bundles.
  - Add a CEL expression safety check to the frontend review: since orbflow uses CEL expressions prefixed with '=' that get evaluated by the engine, the review prompt should flag any user-provided CEL expressions rendered without sanitization or any CEL strings constructed via string concatenation instead of the cel-builder utility.

### Prompt Template Fragments


  - ## Persona
You are a senior frontend engineer specializing in React 19, Next.js 15 App Router, TypeScript (strict mode), and Zustand state management. You review code for correctness, performance, security, and accessibility -- not style (ESLint handles that).
  - ## Severity Levels
Rate each finding:
- **CRITICAL**: Security vulnerability (XSS, injection), data loss, application crash
- **HIGH**: Correctness bug, significant performance regression, broken accessibility
- **MEDIUM**: Maintainability concern, minor performance issue, incomplete error handling
- **LOW**: Suggestion for improvement, alternative approach worth considering

Only flag issues at MEDIUM or above unless specifically asked for LOW-level suggestions.
  - ## Component Architecture Checks
- Components should have a single responsibility. Flag components exceeding 200 lines or mixing data fetching, business logic, and presentation.
- Verify proper separation: container components (data/logic) vs presentational components (pure rendering).
- Check that shared components in packages/orbflow-core export clean interfaces with well-typed props.
- Flag prop drilling deeper than 3 levels -- suggest Zustand store or React context instead.
  - ## Hooks Correctness
- useEffect dependency arrays must include ALL referenced reactive values. Flag any eslint-disable of exhaustive-deps.
- useEffect must have cleanup functions for subscriptions, timers, event listeners, and abort controllers.
- Custom hooks must start with 'use' and follow Rules of Hooks (no conditional calls).
- useMemo/useCallback: flag if used without measured performance need (premature optimization), but also flag missing memoization on expensive computations or callbacks passed to memoized children.
  - ## Zustand Store Review
- Flag: `const { x, y, z } = useStore()` without useShallow -- subscribes to entire store.
- Prefer: `const x = useStore(s => s.x)` for single values.
- Prefer: `const { x, y } = useStore(useShallow(s => ({ x: s.x, y: s.y })))` for multiple values.
- Flag store actions that mutate state directly instead of using set() with a new object.
- Flag computed values in stores that should be derived selectors instead.
  - ## Next.js App Router Patterns
- Verify 'use client' is only on components that need interactivity (onClick, useState, useEffect).
- Flag large component trees marked as client when only a small child needs interactivity.
- Check that data fetching uses server components, server actions, or route handlers -- not client-side fetch for initial data.
- Flag missing loading.tsx, error.tsx, or not-found.tsx in route segments.
- Verify proper use of next/dynamic for lazy-loading heavy client components.
  - ## Frontend Security
- **CRITICAL**: Flag any dangerouslySetInnerHTML without DOMPurify.sanitize().
- **CRITICAL**: Flag href={userInput} without URL validation (javascript: protocol injection).
- **CRITICAL**: Flag eval(), new Function(), or document.write() with dynamic content.
- **HIGH**: Flag sensitive data in localStorage/sessionStorage (use httpOnly cookies).
- **HIGH**: Flag missing origin checks on postMessage listeners.
- **HIGH**: Flag CEL expressions constructed via string concatenation instead of the cel-builder utility.
  - ## Accessibility (WCAG 2.1 AA)
- All interactive elements must have accessible names (visible text, aria-label, or aria-labelledby).
- Images: informational images need descriptive alt; decorative images need alt="".
- Custom interactive elements must use semantic HTML (<button>, <a>) or have role + keyboard handlers.
- Modals/drawers must implement focus trapping and restore focus on close.
- Form inputs must have associated <label> elements or aria-label.
- Color must not be the only means of conveying information (e.g., status indicators need icons or text too).
  - ## Output Format
For each finding, provide:
```
### [SEVERITY] Category: Brief title
**File**: path/to/file.tsx:lineNumber
**Issue**: What is wrong and why it matters.
**Fix**:
```diff
- problematic code
+ corrected code
```
```

### Sources


  - https://5ly.co/blog/ai-prompts-for-code-review/ - AI Prompts for Code Review: comprehensive prompt templates for architecture, security, performance (2026)
  - https://docsbot.ai/prompts/programming/react-nextjs-code-review - React Next.js Code Review system prompt template
  - https://pagepro.co/blog/18-tips-for-a-better-react-code-review-ts-js/ - 18-point React/TypeScript code review checklist
  - https://crashoverride.com/blog/prompting-llm-security-reviews - How to Prompt LLMs for Better Security Reviews (PCEI framework)
  - https://addyosmani.com/blog/ai-coding-workflow/ - Addy Osmani's LLM coding workflow for 2026
  - https://www.perssondennis.com/articles/react-anti-patterns-and-best-practices-dos-and-donts - React Anti-Patterns and Best Practices
  - https://jsdev.space/react-anti-patterns-2025/ - 15 React Anti-Patterns and Fixes (2025)
  - https://itnext.io/6-common-react-anti-patterns-that-are-hurting-your-code-quality-904b9c32e933 - 6 Common React Anti-Patterns Hurting Code Quality
  - https://github.com/pmndrs/zustand/discussions/1916 - Zustand: selecting multiple store props best practice
  - https://deepwiki.com/pmndrs/zustand/2.3-selectors-and-re-rendering - Zustand Selectors and Re-rendering patterns
  - https://nextjs.org/docs/app/guides/production-checklist - Next.js Production Checklist (official)
  - https://nextjs.org/docs/app/getting-started/server-and-client-components - Next.js Server and Client Components (official)
  - https://www.invicti.com/blog/web-security/is-react-vulnerable-to-xss - React XSS vulnerability patterns
  - https://pragmaticwebsecurity.com/articles/spasecurity/react-xss-part2 - Preventing XSS in React: dangerouslySetInnerHTML
  - https://dev.to/sathish_daggula/cursor-claude-my-ai-code-review-checklist-hm5 - Cursor + Claude AI code review checklist
  - https://code.claude.com/docs/en/code-review - Claude Code official code review documentation
  - https://github.com/anthropics/claude-code/blob/main/plugins/code-review/README.md - Claude Code review plugin (multi-agent parallel review)
  - https://www.seangoedecke.com/ai-agents-and-code-review/ - AI agents and code review: effective prompting principles
  - https://itnext.io/instantly-boost-your-coding-agents-performance-with-3-simple-prompts-ceb4dc9b5f05 - Boost coding agent performance with structured prompts
  - https://microsoft.github.io/code-with-engineering-playbook/code-reviews/recipes/javascript-and-typescript/ - Microsoft Engineering Playbook: JS/TS Code Reviews

### Relevance To Orbflow


  > Orbflow's frontend is a Next.js 15 App Router application (apps/web) with a headless SDK package (packages/orbflow-core) using Zustand for state management across four core stores (canvasStore, workflowStore, executionOverlayStore, credentialStore). This architecture creates several review-critical surface areas that generic prompts miss entirely. First, the Zustand stores are the central nervous system of the workflow builder -- improper selector usage causes cascading re-renders across the canvas, node picker, and config modals, making Zustand-specific review checks essential. Second, the monorepo split between orbflow-core (zero CSS, headless) and apps/web (visual components) means review prompts must enforce that orbflow-core never imports UI-specific dependencies and that apps/web components use orbflow-core's exported interfaces correctly. Third, orbflow uses CEL expressions extensively for dynamic workflow values, creating a unique frontend security surface: CEL strings prefixed with '=' flow from user input through the frontend builder to engine evaluation, so the review must check that cel-builder utilities are used instead of raw string concatenation and that CEL expressions displayed in the execution viewer are properly sanitized.

The frontend also handles sensitive credential management (credential-manager components with AES-256-GCM encrypted storage), approval gates for workflow execution, and real-time execution visualization with streaming data -- all areas where standard React review prompts provide insufficient coverage. A orbflow-specific frontend review prompt must address these domain concerns alongside standard React/Next.js/TypeScript/Zustand best practices to be effective.

### Automation Hooks


  - Create a PostToolUse hook on file write/edit that triggers a focused frontend review sub-agent when .tsx/.ts files in apps/web/ or packages/orbflow-core/ are modified. The hook should pass the git diff of changed files to the review prompt automatically.
  - Configure a Stop hook that runs a frontend review sub-agent on all modified frontend files before the session ends. The sub-agent uses the REVIEW-FRONTEND.md prompt and outputs findings as GitHub-style inline comments on the diff.
  - Use Claude Code's parallel sub-agent pattern to run 4 specialized review agents simultaneously: (1) hooks-correctness + render-performance, (2) Zustand store patterns, (3) security + accessibility, (4) Next.js patterns + bundle size. Each agent gets a focused prompt slice and a confidence threshold (>=80) for reporting.
  - Add a REVIEW.md file to the repository root with frontend-specific review rules that Claude Code automatically discovers. Include the prompt template fragments for component architecture, Zustand patterns, and security checks so they are applied on every review without manual prompt entry.
  - Integrate with git diff via a pre-commit or pre-push hook script that invokes Claude Code with the frontend review prompt on staged .tsx/.ts files. Use the --output-format json flag to produce machine-parseable findings that can block the commit if CRITICAL issues are found.
  - Create a slash command (/review-frontend) as a Claude Code skill that loads the frontend review prompt template, automatically scopes to apps/web/ and packages/orbflow-core/ workspaces, and runs the review against the current git diff. The skill should accept optional flags like --security-only or --perf-only to run focused sub-reviews.

---

## 4. Generic Implementation Review Template Design

### Key Principles


  - Parameterize all context inputs: inject git diff, changed files list, implementation plan summary, and architecture constraints as template variables rather than hardcoding any feature-specific content. Use shell-expandable placeholders like !`git diff main...HEAD` for dynamic injection.
  - Enforce structured output schema: every finding must include severity (critical/warning/suggestion), location (file:line), issue description, and a concrete code fix. Prohibit vague suggestions like 'consider adding error handling' — require actionable snippets.
  - Scope reviews dynamically to changed files only: use git diff --name-only to build the file list, then filter by glob patterns (e.g., src/**/*.rs for backend, apps/web/**/*.tsx for frontend) to route findings to the correct review domain.
  - Use multi-pass review architecture: first pass estimates scope across changed files, second pass groups related files within token limits, third pass performs deep analysis per group, fourth pass synthesizes cross-file insights.
  - Separate 'what changed' from 'reference context' explicitly: use XML-style tags like <diff type="changed"> vs <context type="reference"> to prevent the LLM from confusing modified code with surrounding context, reducing hallucinated file references.
  - Exclude style/formatting findings by default: explicitly instruct the reviewer to skip style preferences and focus only on correctness, security, performance, and logic errors. This dramatically reduces noise in review output.
  - Use few-shot examples in the template for severity calibration: include 2-3 example findings with their severity ratings so the LLM learns your team's severity scale (e.g., hardcoded credentials = Critical, missing JSDoc = Low).
  - Chunk large diffs by file or module: for diffs exceeding ~30k tokens, split into file-level or module-level chunks and add the instruction 'Do not assume context outside this chunk; note suspected cross-file risks briefly.'
  - Template composability over monolithic prompts: build templates from reusable fragments (severity schema, output format, focus areas) that can be assembled per review type rather than maintaining one giant prompt.
  - Include implementation plan as verification context: inject a summary of the implementation plan so the reviewer can verify that changes align with intended design, catching drift between plan and execution.

### Concrete Examples


  - **name:** Parameterized Review Template with Dynamic Injection | **before:** Review the workflow engine changes for bugs and security issues. | **after:** You are reviewing changes from an implementation wave. Here is the context:

<implementation_plan>
{{PLAN_SUMMARY}}
</implementation_plan>

<changed_files>
{{CHANGED_FILES_LIST}}
</changed_files>

<diff>
{{GIT_DIFF}}
</diff>

For each issue found, output EXACTLY this format:
## <n>) Severity: CRITICAL|HIGH|MEDIUM|LOW | Type: Bug|Security|Performance|Logic|Maintainability
Title: <short imperative>

Affected:
- path/to/file.ext:lineStart-lineEnd

Explanation:
<what is wrong, why it matters>

Proposed fix:
```<lang>
<minimal code snippet>
```

Do NOT comment on style or formatting. Do NOT echo the diff back. Do NOT add preambles or conclusions.
  - **name:** Dynamic Scoping with Glob Filters | **before:** Review all the code changes. | **after:** # Step 1: Identify scope
Changed files: !`git diff main...HEAD --name-only`

# Step 2: Route to domain
Backend files (crates/**/*.rs): Review for port/adapter compliance, error handling via OrbflowError, immutability.
Frontend files (apps/web/**/*.tsx): Review for Zustand store patterns, React best practices, accessibility.
Shared types (packages/orbflow-core/**/*.ts): Review for API contract compatibility, type safety.

# Step 3: Review only files matching the current domain filter
Skip files without actual diff hunks.
  - **name:** Severity Calibration via Few-Shot Examples | **before:** Rate each issue by severity. | **after:** Rate each issue using this severity scale. Examples:

Example 1:
Issue: SQL query built with string concatenation from user input
Severity: CRITICAL
Reasoning: Direct SQL injection vulnerability in production endpoint

Example 2:
Issue: Unused import left in file
Severity: LOW
Reasoning: No functional impact, cosmetic only

Example 3:
Issue: Mutex held across an await point in async Rust code
Severity: HIGH
Reasoning: Can cause deadlocks under concurrent load

Now classify each finding in the diff using the same format.
  - **name:** Implementation Plan Alignment Check | **before:** Check if the code looks correct. | **after:** The implementation plan specified:
{{PLAN_REQUIREMENTS}}

Verify:
1. Are all planned changes present in the diff? List any MISSING items.
2. Are there UNPLANNED changes not described in the plan? Flag as 'scope creep'.
3. Does the implementation match the planned approach, or has it drifted?
4. Are error handling and edge cases from the plan addressed?

Output a checklist with status: DONE | MISSING | DRIFTED | EXTRA for each planned item.
  - **name:** Composable Template Fragment Assembly | **before:** One large monolithic review prompt. | **after:** # Template fragments (combine as needed):

## fragment: output-format.md
For each issue: Severity | Type | Title | Affected files:lines | Explanation | Proposed fix

## fragment: focus-security.md
Focus on: OWASP top 10, credential exposure, injection, auth bypass, rate limiting

## fragment: focus-rust-backend.md
Focus on: async safety, error propagation via OrbflowError, trait implementation correctness, unsafe blocks

## fragment: focus-react-frontend.md
Focus on: hook rules, state management patterns, XSS via dangerouslySetInnerHTML, accessibility

## fragment: plan-alignment.md
[plan alignment check template]

# Assembly: cat output-format.md focus-rust-backend.md plan-alignment.md > review-prompt.md

### Anti Patterns


  - **pattern:** Hardcoding feature names or file paths in the review template | **why_it_fails:** Makes the template single-use. Every new feature requires editing the prompt, defeating the purpose of a generic template. | **instead:** Use parameterized placeholders like {{CHANGED_FILES}}, {{GIT_DIFF}}, {{PLAN_SUMMARY}} that are populated at invocation time via shell expansion or template engines.
  - **pattern:** Asking for review without structured output format | **why_it_fails:** LLMs produce inconsistent, narrative-style feedback that is hard to parse, track, or act on. Findings get buried in prose. | **instead:** Define an explicit output schema with numbered issues, each containing severity, location, description, and concrete fix. Prohibit preambles, conclusions, and diff echoing.
  - **pattern:** Reviewing the entire codebase instead of scoping to changed files | **why_it_fails:** Wastes token budget on unchanged code, produces irrelevant findings about pre-existing issues, and dilutes focus from the actual changes being reviewed. | **instead:** Use git diff --name-only to identify changed files, filter by domain-specific globs, and only include diff hunks in the review context.
  - **pattern:** Including style and formatting feedback alongside correctness issues | **why_it_fails:** Style comments (variable naming preferences, brace placement) create noise that drowns out critical bugs and security issues. Developers learn to ignore the output. | **instead:** Explicitly exclude style feedback in the prompt: 'Do NOT comment on style preferences or formatting. Only flag issues that affect correctness, security, or performance.' Use linters for style enforcement.
  - **pattern:** Single-pass review of large diffs without chunking | **why_it_fails:** LLMs lose focus and accuracy on large inputs. Findings become shallow or miss issues in later files. Token limits may truncate important context. | **instead:** Chunk diffs by file or module (targeting ~30k tokens per chunk), review each chunk independently, then run a synthesis pass to catch cross-file issues.
  - **pattern:** Vague severity labels without calibration examples | **why_it_fails:** Without examples, 'HIGH' severity means different things to different prompt runs. The LLM may flag minor issues as critical or underrate real vulnerabilities. | **instead:** Include 2-3 few-shot examples that demonstrate your severity scale with concrete reasoning, so the LLM calibrates its classifications consistently.

### Implementation Recommendations


  - Create a review-templates/ directory in the orbflow-rust repo with composable fragments: output-format.md, focus-rust-backend.md, focus-react-frontend.md, focus-shared-types.md, plan-alignment.md. Assemble these dynamically based on which files changed in the diff.
  - Use git diff --name-only main...HEAD piped through grep patterns to auto-detect whether the review should use the Rust backend template (crates/**), the React frontend template (apps/web/**), or the shared SDK template (packages/orbflow-core/**). Multiple templates can fire in parallel for cross-domain changes.
  - Build a review.sh shell script that: (1) captures git diff main...HEAD, (2) captures git diff --name-only main...HEAD, (3) reads the implementation plan summary from a known location (e.g., docs/design/current-plan.md), (4) assembles the appropriate template fragments, (5) injects all variables, and (6) invokes Claude Code with the composed prompt.
  - For orbflow's ports-and-adapters architecture, add a backend-specific checklist fragment that verifies: port trait implementations match orbflow-core signatures, OrbflowError variants are used correctly, no cross-crate boundary violations, wire types maintain JSON snake_case compatibility.
  - For orbflow's frontend, add a frontend-specific checklist fragment that verifies: Zustand store immutability patterns, proper use of orbflow-core SDK exports, no direct API calls bypassing createApiClient(), React hook rules compliance, and accessibility standards.
  - Integrate review templates with Claude Code hooks: create a PostToolUse hook that automatically triggers the appropriate review template after git commit operations, or create a /review slash command that accepts an optional --plan flag to include plan-alignment checking.
  - Store severity calibration examples specific to orbflow's codebase: e.g., 'Mutating Instance in place instead of creating a new copy = HIGH (violates immutability principle)', 'Missing rate limiting on new HTTP endpoint = CRITICAL', 'Unused import = LOW'.

### Prompt Template Fragments


  - ## OUTPUT FORMAT
For each issue found, provide EXACTLY this structure:

## <n>) Severity: CRITICAL|HIGH|MEDIUM|LOW | Type: Bug|Security|Performance|Logic|Maintainability
Title: <short imperative description>

Affected:
- path/to/file.ext:lineStart-lineEnd

Explanation:
<What is wrong and why it matters. Reference specific code from the diff.>

Proposed fix:
```<lang>
<minimal code snippet showing the corrected code>
```

Rules:
- Do NOT echo the diff back
- Do NOT add preambles or conclusions
- Do NOT comment on style or formatting
- Only flag issues affecting correctness, security, or performance
- If no issues found, output exactly: "No issues found."
  - ## DYNAMIC CONTEXT INJECTION
You are reviewing changes for the orbflow workflow automation platform.

Branch: !`git branch --show-current`
Changed files:
!`git diff main...HEAD --name-only`

Diff:
```
!`git diff main...HEAD`
```

Implementation plan summary:
!`cat docs/design/current-plan.md 2>/dev/null || echo 'No plan file found'`
  - ## DOMAIN ROUTING
Based on changed files, apply the relevant review focus:

IF crates/**/*.rs changed:
- Verify port trait compliance with orbflow-core definitions
- Check OrbflowError usage (correct variants, no raw strings)
- Ensure immutable patterns (new Instance copies, not mutation)
- Validate async safety (no mutex held across .await)
- Check wire type JSON field naming (snake_case)

IF apps/web/**/*.tsx changed:
- Verify Zustand store immutability (no direct state mutation)
- Check React hook rules (no conditional hooks)
- Validate accessibility (aria labels, keyboard navigation)
- Ensure API calls go through createApiClient()

IF packages/orbflow-core/**/*.ts changed:
- Verify type exports match API contract
- Check for breaking changes to public API surface
- Validate store action signatures
  - ## PLAN ALIGNMENT CHECK
The implementation plan for this wave specified:
{{PLAN_SUMMARY}}

For each planned item, verify:
| Planned Item | Status | Evidence |
|---|---|---|
| <item> | DONE / MISSING / DRIFTED / EXTRA | <file:line or explanation> |

Flag any changes in the diff that are NOT described in the plan as UNPLANNED.
  - ## SEVERITY CALIBRATION (orbflow-specific)
Use these examples to calibrate your severity ratings:

CRITICAL:
- SQL injection in orbflow-postgres query building
- Credential encryption key exposed in config
- Bus message deserialization allowing arbitrary code execution

HIGH:
- Mutating Instance in-place instead of creating new copy
- Missing authentication check on HTTP API endpoint
- Deadlock risk from mutex held across async .await
- CEL expression injection without sanitization

MEDIUM:
- Missing error variant in OrbflowError for new failure mode
- No retry logic on transient NATS connection failure
- React component missing error boundary

LOW:
- Unused import or variable
- Minor naming inconsistency
- Missing doc comment on internal function

### Sources


  - https://github.com/nurettincoban/diff2ai - diff2ai: tool for generating AI code review prompts from git diffs with customizable templates
  - https://graphite.com/guides/effective-prompt-engineering-ai-code-reviews - Graphite guide on effective prompt engineering for AI code reviews
  - https://www.anthonybordonaro.com/ai-toolkit/code-review-prompt - Structured code review prompt with severity/location/fix format
  - https://github.com/disler/diffbro - diffbro: CLI tool using git diff with GPT for code review
  - https://odsc.medium.com/context-engineering-for-ai-code-reviews-fix-critical-bugs-with-outside-diff-impact-slicing-6d1ec2fc87e9 - Context engineering for AI code reviews with outside-diff impact slicing
  - https://taskautomation.dev/blog/code-review - Guide on automating code review with git hooks
  - https://5ly.co/blog/ai-prompts-for-code-review/ - AI prompts for code review with structured output patterns
  - https://www.josecasanova.com/blog/claude-code-review-prompt - Simple Claude code review prompt design
  - https://www.eesel.ai/blog/claude-code-best-practices - Claude Code best practices for 2026
  - https://dev.to/pwd9000/mastering-code-reviews-with-github-copilot-the-definitive-guide-3nfp - Mastering code reviews with GitHub Copilot
  - https://nikiforovall.blog/productivity/2025/05/03/github-copilot-prompt-engineering-code-review.html - Code review with GitHub Copilot prompt engineering
  - https://www.365iwebdesign.co.uk/news/2026/01/29/how-to-use-dynamic-context-injection-claude-code/ - Dynamic context injection in Claude Code
  - https://modelcontextprotocol.io/specification/2025-06-18/server/prompts - MCP prompt specification for parameterized templates
  - https://arxiv.org/html/2505.16339v1 - Rethinking Code Review Workflows with LLM Assistance (2025)
  - https://arxiv.org/html/2404.18496v2 - AI-powered Code Review with LLMs: Early Results

### Relevance To Orbflow


  > Orbflow's architecture as a distributed workflow engine with a strict ports-and-adapters pattern makes generic review templates especially valuable. The codebase spans three distinct domains — Rust backend crates (engine, stores, bus, builtins), a React/Next.js frontend (apps/web), and a shared TypeScript SDK (packages/orbflow-core) — each with different review concerns. A parameterized template system allows a single review invocation to dynamically route to domain-specific checklists based on which files changed in the diff. For example, changes to orbflow-engine need verification of DAG coordination and saga compensation logic, while changes to orbflow-httpapi need rate limiting and response envelope compliance checks, and frontend changes need Zustand immutability and accessibility verification. The implementation plan alignment feature is particularly relevant because orbflow uses phased implementation waves (as seen in the docs/design/ directory), and review templates can automatically verify that each wave's changes match the planned scope without requiring manual prompt editing per wave.

### Automation Hooks


  - Create a Claude Code slash command /review that assembles the appropriate template fragments based on git diff --name-only output, injects the diff and plan summary, and runs the review. Usage: /review or /review --plan docs/design/current-wave.md
  - Use Claude Code's !`command` dynamic context injection syntax to populate template variables at invocation time: !`git diff main...HEAD` for the diff, !`git diff main...HEAD --name-only` for the file list, !`git branch --show-current` for branch context.
  - Implement a PostToolUse hook on git commit that triggers a lightweight review (LOW and MEDIUM severity only) as a pre-push quality gate, catching issues before they reach PR review.
  - Build a review.sh script that: (1) detects changed domains via file path patterns, (2) selects and concatenates the appropriate template fragments, (3) injects git diff and plan summary, (4) pipes to Claude Code via stdin. Wire this into CI as a GitHub Action step.
  - Use .aidiffignore-style patterns (similar to diff2ai) to exclude noise files from review scope: **/*.lock, **/dist/**, **/*.generated.ts, pnpm-lock.yaml, Cargo.lock.
  - Create a beforesave hook or pre-commit hook that runs a scoped mini-review on staged files only, using git diff --cached as the input, providing immediate feedback before the full PR review.
  - Store review results as structured JSON (matching the output format template) in a .reviews/ directory, enabling trend analysis of issue types and severity across waves.

---

## 5. Prompt Engineering Anti-Patterns for Code Review

### Key Principles


  - Scope reviews to the diff, not the entire codebase. Feeding whole files causes hallucinated context and dilutes focus. Use `git diff main...HEAD` as the primary input, supplemented with targeted file reads only for imports and type definitions referenced by changed lines.
  - Always enforce a structured output format with mandatory fields (severity, location, issue, fix). Without format constraints, LLMs produce polite but useless prose like 'looks good, consider adding error handling.'
  - Demand concrete code snippets in every finding. A review comment without a proposed fix is noise. Require verbatim code excerpts for evidence and specific replacement code for remediation.
  - Apply prompt routing: use lightweight static analysis (linting, type-checking) to pre-filter which categories of issues to ask the LLM about. This reduced false positives from 225 to 20 in the SAST-Genius study.
  - Avoid overcorrection bias from overly detailed prompts. Research (arXiv 2603.00539) shows that requiring explanations and suggested corrections in complex prompts makes LLMs more prone to flagging correct code as erroneous.
  - Force the LLM to read source files before commenting. Prompts that don't require reading actual imports, type definitions, and surrounding context produce surface-level findings that miss architectural issues.
  - Explicitly exclude style/formatting comments. Without this constraint, LLMs fill reviews with low-value style nitpicks that drown out real bugs and security issues.
  - Use persona-specific prompts rather than generic checklists. A security-focused persona finds different issues than a performance-focused one. Generic OWASP checklists without code context produce boilerplate findings.
  - Limit review scope per pass. For diffs over 500 lines, review file-by-file. LLMs lose coherence on large inputs and fall back to shallow pattern matching.
  - Include a validation phase. After generating findings, have a second pass cross-check each finding against the actual codebase to filter hallucinations before presenting to developers.

### Concrete Examples


  - **label:** Anti-pattern: Unstructured review prompt | **before:** Review this code for issues and let me know what you find. | **after:** Review the following code. For each issue found, provide:
1. **Severity** (critical / warning / suggestion)
2. **Location** (file and line reference)
3. **Issue** (what's wrong)
4. **Fix** (concrete code change)

Focus on: bugs and logic errors, security vulnerabilities, performance issues. Do NOT comment on style preferences or formatting.
  - **label:** Anti-pattern: Reviewing entire codebase instead of changes | **before:** Review all the code in this repository for security vulnerabilities. | **after:** Review only the changes in `git diff main...HEAD`. For each changed function, read the full file to understand context, then check:
- Does the change introduce a new attack surface?
- Are inputs from the diff properly validated?
- Do error paths leak sensitive data?
Ignore unchanged code unless directly called by changed code.
  - **label:** Anti-pattern: Generic OWASP checklist without context | **before:** Check this code against the OWASP Top 10. | **after:** You are a security reviewer for a Rust/Axum HTTP API that handles workflow automation. Review the following diff for:
- SQL injection in any raw query construction (we use sqlx with parameterized queries)
- Authentication bypass in middleware changes
- Unsafe deserialization of user-provided JSON workflow definitions
- Credential leakage in error responses or logs
Ignore OWASP categories not applicable to this stack (e.g., XXE in a JSON-only API).
  - **label:** Anti-pattern: No code snippet requirement | **before:** List any problems you see in this pull request. | **after:** For each finding, you MUST include:
- EVIDENCE: the exact code lines (quoted verbatim) that exhibit the issue
- REMEDIATION: a concrete code patch showing the fix
- IMPACT: one sentence describing what can go wrong if unfixed

If you cannot quote specific code lines, do not report the finding.
  - **label:** Anti-pattern: Single monolithic review pass | **before:** Do a comprehensive code review covering security, performance, correctness, and style. | **after:** Pass 1 - Correctness: Review for logic bugs, off-by-one errors, null/None handling, and missing edge cases.
Pass 2 - Security: Review for injection, auth bypass, data leakage, and unsafe operations.
Pass 3 - Performance: Review for N+1 queries, unnecessary allocations, and blocking calls in async contexts.

Run each pass separately. Do not mix categories.

### Anti Patterns


  - **name:** Over-broad scoping | **description:** Asking the LLM to review an entire repository or all files instead of focusing on the actual changes (diff). This overwhelms the context window, causes the model to lose coherence, and produces shallow pattern-matching findings instead of deep analysis. | **why_it_fails:** Context windows are finite. A 1000-line diff fills the window, leaving no room for reasoning. The model falls back to generic observations about the first and last sections, missing the middle entirely. | **what_to_do_instead:** Scope to `git diff` output. For large diffs, split by file. Supplement with targeted reads of imported types and called functions only.
  - **name:** Generic OWASP/security checklists without code context | **description:** Prompting with 'check against OWASP Top 10' without specifying the technology stack, framework, or which categories are relevant to the codebase. | **why_it_fails:** The LLM generates boilerplate findings for all 10 categories regardless of applicability. A Rust API will never have XXE vulnerabilities, but the LLM will report them anyway. This floods the output with false positives and erodes developer trust. | **what_to_do_instead:** Specify the exact stack (language, framework, database), list only applicable vulnerability categories, and describe the application's threat model.
  - **name:** Prompts that don't force reading source files | **description:** Pasting only a diff or code snippet without instructing the LLM to read imports, type definitions, and surrounding context from the actual files. | **why_it_fails:** The LLM cannot verify whether a variable is properly typed, whether an imported function handles errors, or whether the calling code expects a different return type. It hallucinates the behavior of unread code. | **what_to_do_instead:** Explicitly instruct: 'Before commenting on any function, read the full file containing it. Follow imports to understand types and error handling contracts. Only comment on behavior you can verify from source.'
  - **name:** Missing output format constraints | **description:** Not specifying how findings should be structured, leading to free-form prose responses. | **why_it_fails:** Without format constraints, LLMs produce walls of text mixing observations, suggestions, compliments, and caveats. Developers cannot quickly triage or act on findings. Critical issues get buried in paragraphs of 'overall the code looks good.' | **what_to_do_instead:** Require a fixed schema per finding: severity level, file:line location, issue description (one sentence), evidence (verbatim code), impact (one sentence), and remediation (code patch).
  - **name:** Not demanding concrete code snippets | **description:** Allowing the LLM to describe issues in abstract terms without quoting the problematic code or showing a fix. | **why_it_fails:** Abstract descriptions like 'consider adding error handling' are not actionable. Developers must re-read the code themselves to figure out where and how. This negates the time-saving benefit of AI review. It also allows the LLM to make claims about code that does not exist (hallucination). | **what_to_do_instead:** Add a hard constraint: 'Every finding MUST include the exact problematic code quoted verbatim and a concrete replacement. If you cannot quote specific lines, do not report the finding.'
  - **name:** Overcorrection bias from complex prompts | **description:** Adding too many requirements, explanations, and correction steps to a single prompt, which causes the LLM to assume flaws exist and suggest unnecessary modifications. | **why_it_fails:** Research shows that detailed prompts requiring explicit explanations and suggested corrections counterintuitively increase misjudgment rates. The model's Requirement Conformance Recognition Rate drops, and correct code is misclassified as erroneous. | **what_to_do_instead:** Keep review prompts focused on one concern at a time. Use multi-pass reviews with separate, simple prompts rather than one complex prompt covering everything.
  - **name:** No validation or cross-check phase | **description:** Presenting raw LLM findings directly to developers without any verification step. | **why_it_fails:** 29-45% of AI-generated security findings contain inaccuracies. After 3-5 false positive incidents, developers stop trusting AI output entirely and begin ignoring valid findings too. This is the trust erosion cycle. | **what_to_do_instead:** Add a validation phase: after generating findings, run a second LLM pass or static analysis check to verify each finding against the actual codebase. Filter out unverifiable claims before presenting results.

### Implementation Recommendations


  - Split review prompts into frontend and backend specializations. The Rust backend (orbflow-engine, orbflow-httpapi, orbflow-postgres) needs prompts focused on: unsafe code, SQL injection via sqlx, error handling with OrbflowError variants, async/await pitfalls, and trait implementation correctness. The TypeScript frontend (apps/web, packages/orbflow-core) needs prompts focused on: React hook dependency arrays, Zustand store immutability, XSS in dynamic rendering, and proper TypeScript type narrowing.
  - Use `git diff main...HEAD` as the canonical input for all review prompts. Pipe this through a pre-filter that separates changes by crate/package so each review pass operates on a focused subset.
  - Implement prompt routing based on file type and changed content. If the diff touches `orbflow-httpapi`, route to the security-focused prompt. If it touches `orbflow-engine/dag.go`, route to the correctness-focused prompt. If it touches React components, route to the UI/UX-focused prompt.
  - Create a validation post-processing step in Claude Code hooks (PostToolUse) that checks each finding against the actual source before displaying it. Require the reviewer to quote verbatim code as evidence — findings without verbatim quotes are automatically filtered.
  - For orbflow's CEL expression evaluation (orbflow-cel), create a specialized prompt that understands the CEL DSL and checks for injection risks in user-provided expressions. Generic security prompts will not catch CEL-specific issues.
  - Enforce the structured output format (severity/location/issue/fix) via a JSON schema in the prompt. Parse the output programmatically and reject malformed responses rather than showing raw text to developers.
  - Limit each review pass to 500 lines of diff. For larger PRs, split by crate (Rust) or by component directory (frontend) and run separate focused reviews.

### Prompt Template Fragments


  - ## Scope Constraint
Review ONLY the code changes shown in the diff below. Do not review unchanged code unless it is directly called by, imported into, or affected by the changed lines. Before commenting on any function, read the full source file to verify your understanding of types, imports, and error handling contracts.
  - ## Output Format (MANDATORY)
For each issue, output exactly this structure:

**[SEVERITY]** CRITICAL | HIGH | MEDIUM | LOW
**[FILE]** filename:line_range
**[ISSUE]** One sentence describing what is wrong.
**[EVIDENCE]** Verbatim code excerpt showing the problem.
**[IMPACT]** One sentence describing what can go wrong.
**[FIX]** Concrete code patch showing the remediation.

Do not include preambles, summaries, or compliments. Output only the findings list. If no issues are found, output: NO_ISSUES_FOUND.
  - ## Exclusions
Do NOT report:
- Style preferences or formatting opinions
- Suggestions to add comments or documentation
- Hypothetical issues that require assumptions about code you have not read
- Issues already caught by the project's linter (clippy for Rust, eslint for TypeScript)

If you cannot quote the exact problematic code from the diff or source files, do not report the finding.
  - ## Backend Review Persona (Rust/Axum)
You are a senior Rust engineer reviewing changes to a distributed workflow engine. The codebase uses: Axum (HTTP), tonic (gRPC), sqlx (PostgreSQL), NATS JetStream (messaging), and a custom CEL evaluator. Focus on:
- Ownership and lifetime correctness
- Unsafe code or raw pointer usage
- SQL injection in raw queries (parameterized queries via sqlx are safe)
- Error propagation: are OrbflowError variants used correctly?
- Async safety: no blocking calls in async contexts
- Race conditions in concurrent instance handling (DashMap + Arc<Mutex>)
  - ## Frontend Review Persona (React/TypeScript)
You are a senior React/TypeScript engineer reviewing changes to a workflow builder UI. The codebase uses: Next.js 15, Zustand (state management), React Flow (graph canvas), and a headless SDK in packages/orbflow-core. Focus on:
- React hook rules violations (conditional hooks, missing deps)
- Zustand store mutations (must be immutable — return new objects)
- XSS risks in dynamically rendered workflow node content
- Type safety: are TypeScript types properly narrowed, no unsafe `any` casts?
- Component prop drilling vs store usage consistency
  - ## Multi-Pass Review Strategy
Perform the review in separate focused passes. Do NOT mix concerns across passes.

Pass 1 — CORRECTNESS: Logic bugs, off-by-one errors, null handling, missing edge cases, incorrect type usage.
Pass 2 — SECURITY: Injection attacks, auth/authz bypass, data leakage in errors/logs, unsafe deserialization.
Pass 3 — PERFORMANCE: N+1 queries, unnecessary clones/allocations, blocking in async, missing indexes.

Report findings from each pass under a separate heading.
  - ## Hallucination Guard
Before reporting any finding:
1. Verify you can see the problematic code in the diff or have read it from the source file.
2. Confirm the issue is not already handled by surrounding code (check error handling, validation, or guard clauses).
3. If you are uncertain whether the issue is real, prefix the finding with [UNCERTAIN] and explain what additional context would confirm or deny it.

Do not fabricate function signatures, variable names, or behavior you have not verified from source.

### Sources


  - https://diffray.ai/blog/llm-hallucinations-code-review/ — Comprehensive study on LLM hallucination rates (29-45% vulnerability rate) and mitigation strategies (96% reduction with combined approaches)
  - https://graphite.com/guides/effective-prompt-engineering-ai-code-reviews — Graphite guide on structured prompts, persona usage, and prompt templates for AI code reviews
  - https://crashoverride.com/blog/prompting-llm-security-reviews — Practical guide on five elements of effective security review prompts: persona, context, examples, specific instructions, output format
  - https://5ly.co/blog/ai-prompts-for-code-review/ — 2026 guide to AI code review prompts covering architecture, security, and anti-pattern detection
  - https://www.anthonybordonaro.com/ai-toolkit/code-review-prompt — Structured code review prompt template requiring severity, location, issue, and fix for every finding
  - https://arxiv.org/html/2603.00539 — Research paper on overcorrection bias: detailed prompts cause LLMs to misclassify correct code as non-conforming
  - https://arxiv.org/html/2510.12186v1 — iCodeReviewer: Mixture of Prompts approach for secure code review with prompt routing
  - https://github.com/nurettincoban/diff2ai — Tool for converting git diffs into focused AI code review prompts
  - https://github.com/baz-scm/awesome-reviewers — Collection of system prompts for agentic code review
  - https://dextralabs.com/blog/ai-driven-code-reviews-prompts/ — DextraLabs prompt strategies emphasizing scope, context, and output constraints
  - https://medium.com/data-science-collective/youre-using-ai-to-write-code-you-re-not-using-it-to-review-code-728e5ec2576e — Seven AI prompts for code review and security audits with structured output
  - https://odsc.medium.com/context-engineering-for-ai-code-reviews-fix-critical-bugs-with-outside-diff-impact-slicing-6d1ec2fc87e9 — Context engineering using impact slicing to catch cross-boundary bugs

### Relevance To Orbflow


  > Orbflow is a distributed workflow automation engine with a Rust backend (ports-and-adapters architecture across 15+ crates) and a React/TypeScript frontend (monorepo with headless SDK). This research directly applies because orbflow's review needs span two fundamentally different technology stacks that require separate, specialized review prompts — generic prompts would miss Rust-specific issues like ownership errors, unsafe async patterns, and OrbflowError variant misuse, while also missing React-specific issues like Zustand store mutations and hook dependency violations. The anti-patterns identified are especially dangerous for orbflow: over-broad scoping would overwhelm reviews of the 15-crate workspace, generic OWASP checklists would miss CEL expression injection risks specific to orbflow-cel, and unstructured output would make findings across orbflow-engine, orbflow-httpapi, and orbflow-postgres impossible to triage. Orbflow's use of DashMap with Arc<Mutex> for per-instance locking, event sourcing with snapshots, and the NodeExecutor trait pattern all require domain-aware review prompts that understand these architectural patterns rather than applying generic code review heuristics.

### Automation Hooks


  - PreToolUse hook on Bash(git commit): Automatically trigger a diff-scoped code review before commits. Extract `git diff --cached` and route it through the appropriate review prompt (Rust or TypeScript) based on file extensions in the diff.
  - PostToolUse hook on Edit/Write: After any file modification, run a lightweight validation pass checking only the modified file against its type-specific review checklist (Rust safety or React hooks rules).
  - Slash command `/review-pr`: Orchestrate a multi-pass review pipeline — (1) extract `git diff main...HEAD`, (2) split by crate/package, (3) run backend review persona on .rs files, (4) run frontend review persona on .ts/.tsx files, (5) aggregate and deduplicate findings, (6) output structured JSON.
  - Slash command `/review-security`: Run security-focused review with orbflow-specific context (CEL injection, credential store encryption, SQL parameterization in orbflow-postgres, CORS/rate-limiting in orbflow-httpapi).
  - Claude Code hook for validation phase: After any review tool generates findings, automatically run a validation sub-agent that cross-checks each finding by reading the referenced source files and confirming the evidence quotes match actual code. Filter findings that fail validation.
  - Git pre-push hook integration: Run `git diff origin/main...HEAD` through the structured review prompt and block push if any CRITICAL severity findings are reported. Output findings to a temporary file for developer review.
  - Automated prompt routing via file-path matching: Configure rules that map changed file paths to specialized review prompts — `crates/orbflow-engine/**` routes to correctness+concurrency prompt, `crates/orbflow-httpapi/**` routes to security+API prompt, `apps/web/src/core/**` routes to React+Zustand prompt.

---

## 6. Rust Backend Code Review Prompt Patterns

### Key Principles


  - Rust-specific review prompts must target what the compiler cannot catch: business logic errors, API design flaws, unnecessary clones, unsafe code without safety contracts, and performance regressions in hot paths. Generic 'review this code' prompts miss Rust-unique concerns.
  - Layered review structure: start with high-level design (does this module/trait need to exist?), then correctness (lifetimes, ownership, Result handling), then idioms/readability, then performance. This mirrors how experienced Rust reviewers think.
  - Unsafe code demands a written safety contract in every review prompt: not what the code does, but WHY it is safe, what invariants are assumed, and under what conditions it would break. No unsafe block should pass review without this documentation.
  - Clone is the duct tape of Rust -- review prompts must explicitly ask reviewers to justify every clone(), especially inside loops or hot paths. Cloning to satisfy the borrow checker is a design smell; consider Cow<T>, borrowing restructuring, or Arc.
  - Trait proliferation is a common Rust anti-pattern: every trait adds complexity. Review prompts should ask whether a trait is justified by generic use, async polymorphism, or test mocking -- not created reflexively as in Java-style DI.
  - Error handling review must distinguish library code (must return Result, never panic) from application code (may panic in truly unrecoverable situations). Prompts should flag unwrap() in non-test code and check for swallowed errors.
  - Async correctness requires checking for blocking operations inside async contexts, proper use of tokio::join!/spawn for concurrency, and ensuring Mutex locks are not held across await points.
  - Crate boundary hygiene: dependencies should point inward (only core types imported across boundaries). Review prompts should verify that adapter crates do not leak implementation details and that port traits define clean interfaces.
  - Concurrency review must go beyond what the compiler checks: deadlock potential (lock ordering), excessive lock hold times, whether Arc<Mutex<T>> is hiding unnecessary shared state, and channel vs shared-state trade-offs.
  - Performance review in Rust requires domain context that clippy cannot provide: holding locks too long, unnecessary collect() on large iterators when streaming would suffice, type conversion churn to appease the type system, and heap allocations in hot paths.

### Concrete Examples


  - **name:** Generic vs Rust-specific review prompt | **before:** Review this code for bugs and best practices. | **after:** Review this Rust code focusing on: (1) ownership -- are there unnecessary clones that could be replaced with borrows or Cow<T>? (2) error handling -- does any non-test code use unwrap() or expect() without justification? (3) unsafe blocks -- does each have an inline safety contract documenting invariants? (4) async correctness -- are there blocking calls inside async fn or Mutex locks held across .await? (5) trait design -- does every trait have more than one implementor or a clear testing/polymorphism justification? | **why:** The specific prompt surfaces Rust-unique concerns that generic prompts completely miss, like clone abuse, unsafe contracts, and async blocking.
  - **name:** Performance-focused hot path review | **before:** Check this code for performance issues. | **after:** Analyze this code path for performance: (1) Identify any clone() calls inside loops or frequently-called functions. (2) Check for collect::<Vec<_>>() on large iterators where a streaming/iterator approach would avoid allocation. (3) Flag any Mutex/RwLock held across await points or longer than necessary. (4) Look for repeated String/Vec allocations that could use pre-allocated buffers or Cow. (5) Check for unnecessary type conversions (e.g., String -> &str -> String round-trips). | **why:** Rust performance regressions often pass all tests and clippy but kill latency SLAs. Specific allocation and locking patterns need explicit prompt guidance.
  - **name:** Unsafe code audit prompt | **before:** Check for unsafe code. | **after:** Audit all unsafe blocks in this diff: For each unsafe block, verify: (1) There is an inline comment explaining the safety contract -- not what the code does, but WHY it is safe. (2) Document what invariants are assumed (alignment, validity of pointers, FFI buffer ownership). (3) Identify whether the unsafe could be eliminated with safe abstractions. (4) Check if the unsafe block's safety depends on constructor invariants or struct field assumptions that could be violated by other code paths. (5) Flag any unsafe in hot paths where the safety assumptions might not hold under all platform conditions (e.g., alignment on Windows vs Linux). | **why:** Unsafe code that 'works on my machine' is a ticking time bomb. Cross-platform invariant checking and constructor-dependency analysis catch real production bugs.
  - **name:** Crate boundary review prompt for ports-and-adapters | **before:** Review the module organization. | **after:** Verify crate boundary hygiene: (1) Does orbflow-core define all port traits and domain types without importing any adapter crate? (2) Do adapter crates (orbflow-postgres, orbflow-natsbus, etc.) only import orbflow-core? (3) Are there any circular or lateral dependencies between adapter crates? (4) Do port trait signatures use domain types only, not adapter-specific types (e.g., sqlx::Row leaking into a trait)? (5) Is wire format serialization (JSON field names, snake_case) consistent between Rust structs and frontend TypeScript types? | **why:** Ports-and-adapters architecture breaks silently when implementation details leak across boundaries. This prompt enforces the architectural invariant that dependencies point inward.
  - **name:** SQL injection prevention in Rust backend | **before:** Check for security issues. | **after:** Map all input paths from HTTP/gRPC handlers to database queries: (1) Verify all SQL uses parameterized queries via sqlx::query! or sqlx::query_as! macros (compile-time checked). (2) Flag any use of format!() or string concatenation to build SQL. (3) Check that user-provided sort/filter fields are validated against an allowlist, not interpolated directly. (4) Verify CEL expression inputs are sandboxed and cannot escape to SQL. (5) Check that pagination parameters (offset, limit) are validated as positive integers before reaching the query layer. | **why:** Rust's type system helps but does not prevent SQL injection when raw query strings are constructed. Compile-time checked macros like sqlx::query! are the gold standard.

### Anti Patterns


  - **pattern:** Treating Rust like any other language in review prompts | **why_it_fails:** Generic prompts like 'review for bugs and best practices' miss Rust-specific concerns: ownership semantics, borrow checker workarounds (unnecessary clones), unsafe code contracts, async blocking, and trait coherence rules. The compiler catches syntax and memory safety -- reviews must catch what it cannot. | **instead:** Always include Rust-specific sections: ownership/borrowing justification, unsafe audit with safety contracts, async correctness checks, and clone/allocation analysis.
  - **pattern:** Assuming the compiler replaces code review | **why_it_fails:** The Rust compiler catches syntactic and memory safety violations, but NOT: business logic errors, performance regressions (clone in hot loops, lock contention), API design problems, unnecessary complexity, or architectural boundary violations. Teams that skip reviews because 'it compiles' accumulate tech debt. | **instead:** Frame the review prompt around 'things the compiler cannot see': design quality, performance cost, abstraction clarity, and production readiness.
  - **pattern:** Reviewing clone() as always acceptable | **why_it_fails:** clone() is often used as duct tape to satisfy the borrow checker. In hot paths, unnecessary clones cause real performance regressions that pass all tests and CI. Reviewers who see clone() and move on miss the root cause. | **instead:** Prompt should ask: Why is this data being cloned? Can this borrow be restructured? Could Cow<T> or Arc<T> be used? Is this clone inside a loop or hot path?
  - **pattern:** Creating traits for every abstraction (Java-style DI) | **why_it_fails:** Rust is not Java. Single-implementor traits add indirection that hides bugs and increases complexity without providing real polymorphism or testability benefits. Over-traiting makes code harder to navigate and review. | **instead:** Prompt should verify: Does this trait have multiple implementors? Is it needed for test mocking? Does it serve async polymorphism? If none, push back on the trait.
  - **pattern:** Ignoring lock hold duration in concurrent code | **why_it_fails:** The Rust compiler prevents data races but cannot detect deadlocks, lock contention, or performance degradation from holding Mutex/RwLock across await points or expensive operations. These issues only manifest under production load. | **instead:** Review prompts must explicitly check: Is any lock held across an .await? Is the critical section minimal? Is lock ordering consistent to prevent deadlocks?
  - **pattern:** Using broad unwrap() without context | **why_it_fails:** unwrap() crashes the process when the Option is None or Result is Err. In library code or production paths, this creates unreliable services. Even expect() without a meaningful message makes debugging difficult. | **instead:** Prompt should flag all unwrap() in non-test code. Require Result propagation with ? operator, or expect() with a message explaining why the value should always be present.

### Implementation Recommendations


  - Create a dedicated Rust backend review prompt file (e.g., review-rust-backend.md) separate from frontend review prompts. Rust review concerns (ownership, unsafe, async, crate boundaries) have zero overlap with TypeScript/React review concerns (component patterns, hook rules, CSS).
  - Structure the Rust review prompt in layers matching the expert review flow: (1) Architecture/Design -- crate boundaries, trait justification, module organization; (2) Correctness -- ownership, lifetimes, error handling, Result propagation; (3) Safety -- unsafe audit, SQL injection, input validation; (4) Performance -- clone analysis, allocation patterns, lock contention, async blocking; (5) Production readiness -- logging, error messages, panic-freedom.
  - For orbflow specifically, include a crate boundary verification section that checks the ports-and-adapters invariant: orbflow-core defines all port traits, adapter crates only import orbflow-core, no lateral dependencies between adapters, wire types use consistent snake_case JSON serialization.
  - Add a orbflow-specific CEL expression security section: verify that user-provided CEL expressions in workflow definitions are sandboxed, cannot access system resources, and do not create injection vectors when evaluated by orbflow-cel.
  - Include an engine concurrency section specific to orbflow's DashMap<InstanceId, Arc<Mutex<()>>> pattern: verify per-instance locking is correct, optimistic retry logic handles all conflict cases, and no lock is held across async boundaries.
  - Reference the existing CLAUDE.md conventions in the review prompt: immutable domain objects, builder patterns (EngineOptionsBuilder), event sourcing with DomainEvent variants, and NodeExecutor trait pattern for builtin nodes.

### Prompt Template Fragments


  - ## Ownership and Borrowing
- For each function: is ownership transfer via move intentional? Could parameters accept &T or &[T] instead of owned types?
- Flag every clone() call: is it necessary, or is it working around a borrow checker issue? Consider Cow<T>, restructured borrows, or Arc<T>.
- Verify lifetime annotations are minimal and correct -- only present when the compiler cannot infer them.
- Check that slices (&[T], &str) are preferred over owned collections (Vec<T>, String) in function parameters.
  - ## Error Handling
- Flag all unwrap() and expect() in non-test code. Library/service code must return Result and propagate errors with ?.
- Verify custom error types exist for domain errors (using thiserror or similar).
- Check that errors are propagated, never silently swallowed. No empty catch blocks or ignored Result values.
- Ensure panic!() is only used for truly unrecoverable programmer errors, never for expected failure cases.
  - ## Unsafe Code Audit
- Every unsafe block MUST have an inline comment documenting: (a) WHY it is safe, (b) what invariants are assumed, (c) under what conditions it would break.
- Check if the unsafe could be eliminated with safe abstractions.
- If safety depends on struct constructor invariants, verify all constructors maintain those invariants.
- Flag unsafe in hot paths where platform-specific assumptions (alignment, endianness) may not hold universally.
  - ## Async Correctness
- Flag any blocking operation (std::fs, std::net, heavy computation) inside async fn -- use tokio::task::spawn_blocking or async equivalents.
- Verify no Mutex (std::sync::Mutex) is held across .await points -- use tokio::sync::Mutex if lock must span await.
- Check that concurrent operations use tokio::join! or tokio::select! rather than sequential .await chains when independence allows.
- Verify cancellation safety: are there resources that leak if a future is dropped mid-execution?
  - ## Crate Boundary Hygiene (Ports & Adapters)
- orbflow-core defines all port traits and domain types. No adapter crate types should appear in port trait signatures.
- Adapter crates import only orbflow-core. No lateral imports between adapters (e.g., orbflow-postgres must not import orbflow-natsbus).
- Wire types (TaskMessage, ResultMessage) use snake_case JSON field names matching frontend TypeScript types in api.ts.
- Check Cargo.toml dependencies: each crate should only depend on what it directly uses.
  - ## Performance Review
- Identify clone() calls inside loops or frequently-called functions. Suggest borrowing or Cow<T> alternatives.
- Flag collect::<Vec<_>>() on large iterators where streaming/iterator chaining would avoid heap allocation.
- Check Mutex/RwLock critical sections: is the lock held for the minimum necessary duration? Any lock held across .await?
- Look for repeated String/Vec allocations that could use pre-allocated buffers with_capacity().
- Verify no unnecessary type conversions (String -> &str -> String round-trips).
  - ## Concurrency and Thread Safety
- Verify Arc<Mutex<T>> usage is justified -- could the design avoid shared mutable state entirely?
- Check lock ordering consistency across the codebase to prevent deadlocks.
- For DashMap usage: verify no iterator is held while inserting/removing (potential deadlock).
- Verify Send + Sync bounds are satisfied for all types crossing thread boundaries.
- Check channel usage: bounded vs unbounded, backpressure handling, and graceful shutdown.
  - ## SQL and Input Validation
- All SQL queries must use parameterized queries (sqlx::query! or sqlx::query_as! for compile-time checking).
- Flag any format!() or string concatenation used to build SQL strings.
- User-provided sort/filter/column names must be validated against an allowlist.
- All HTTP/gRPC input must be validated at the handler boundary before reaching business logic.
- Pagination parameters (offset, limit) must be validated as bounded positive integers.

### Sources


  - https://pullpanda.io/blog/rust-code-review-checklist - Comprehensive Rust code review checklist covering ownership, borrowing, error handling, traits, concurrency
  - https://bito.ai/blog/rust-code-review/ - Field guide for Rust code review with emphasis on what the compiler cannot catch
  - https://5ly.co/blog/ai-prompts-for-code-review/ - AI prompts for code review including SQL injection, security, architecture review templates
  - https://thepromptshelf.dev/rules/lang/rust/ - Curated Rust AI coding rules from major projects (Zed, OpenAI Codex, Deno)
  - https://www.josecasanova.com/blog/claude-code-review-prompt - Practical Claude Code review prompt focused on critical issues only
  - https://medium.com/rustaceans/6-essential-agent-prompts-for-rust-code-optimization-31428712fcdf - Agent prompts for Rust code optimization
  - https://github.com/ZhangHanDong/rust-code-review-guidelines - Rust Code Review Guidelines (RCRG) open source project
  - https://nnethercote.github.io/perf-book/linting.html - The Rust Performance Book on clippy linting for performance
  - https://www.stackhawk.com/blog/rust-sql-injection-guide-examples-and-prevention/ - Rust SQL injection prevention guide
  - https://github.com/Ranrar/rustic-prompt - Collection of AI instructions for the Rust programming language
  - https://arxiv.org/abs/2504.21312 - Academic paper on annotating and auditing safety properties of unsafe Rust

### Relevance To Orbflow


  > Orbflow's Rust backend is a textbook ports-and-adapters architecture with orbflow-core defining all port traits (Engine, Store, Bus, NodeExecutor, CredentialStore) and adapter crates implementing them. This makes crate boundary hygiene a first-class review concern -- any leakage of adapter types into port traits breaks the architectural invariant. The engine's use of DashMap<InstanceId, Arc<Mutex<()>>> for per-instance locking, combined with async task dispatching via NATS, creates exactly the kind of concurrency pattern where lock-across-await and deadlock issues are most dangerous. The CEL expression evaluator (orbflow-cel) processes user-provided workflow expressions, making it a potential injection vector that needs explicit security review. The builtin node convention (NodeExecutor trait + NodeSchemaProvider) establishes a pattern that review prompts should verify for consistency. Event sourcing with DomainEvent variants and periodic snapshots means crash recovery correctness is a review concern that generic prompts would never surface.

The immutable domain object pattern (engine creates new Instance copies rather than mutating) aligns with Rust's ownership model but requires review attention to ensure clones are intentional and not accidentally expensive. The wire compatibility requirement (snake_case JSON matching frontend api.ts types) is a cross-boundary concern that spans both Rust and TypeScript review prompts, making it important to coordinate between the backend and frontend review files.

### Automation Hooks


  - Claude Code PreToolUse hook: Before any git commit touching crates/ directory, automatically run 'cargo clippy --workspace -- -D warnings' and 'cargo test --workspace' to catch basic issues before review.
  - PostToolUse hook on file edits in crates/orbflow-core/src/ports/: Verify that no adapter-specific types (sqlx, nats, etc.) are imported in port trait definitions. Alert if dependency direction is violated.
  - Git diff integration: Parse 'git diff --name-only' to determine which crates are modified, then select relevant review prompt sections (e.g., only include async correctness for orbflow-engine changes, SQL injection for orbflow-postgres changes).
  - Slash command '/review-rust': Trigger a focused Rust backend review using the layered prompt structure -- runs clippy first, then applies the AI review prompt with crate-specific sections based on changed files.
  - CI GitHub Action using claude-code-action: Configure direct_prompt with the Rust-specific review template fragments, scoped to only review .rs files in the PR diff. Use the concise output format (flag critical issues only, approve otherwise).
  - PreCommit hook: Check for any new unsafe blocks in the diff and require inline safety contract comments before allowing the commit. Parseable via regex matching 'unsafe {' without a preceding '//' comment line.
  - Cargo.toml dependency audit hook: On any Cargo.toml change, verify that adapter crates only add orbflow-core as an internal dependency and flag any lateral adapter-to-adapter dependency additions.

---

