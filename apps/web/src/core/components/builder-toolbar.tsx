"use client";

import { useState, useRef, useEffect, useCallback, type ButtonHTMLAttributes } from "react";
import { cn } from "@/lib/cn";
import { NodeIcon } from "./icons";
import { Tooltip } from "./tooltip";
import { useCanvasStore } from "@/store/canvas-store";
import { useHistoryStore } from "@/store/history-store";

type TriggerType = "manual" | "webhook" | "cron" | "event";

interface TriggerInfo {
  webhookPath?: string;
  cronExpression?: string;
  eventName?: string;
}

export interface ViewControls {
  onAddAnnotation?: (type: "sticky_note" | "text") => void;
  onShowShortcuts?: () => void;
  onToggleSearch?: () => void;
  onToggleGrid?: () => void;
  snapToGrid?: boolean;
}

interface ToolbarProps {
  workflowName: string;
  onNameChange: (name: string) => void;
  workflowDescription?: string;
  onDescriptionChange?: (desc: string) => void;
  onSave: () => void;
  onRun?: () => void;
  onUndo: () => void;
  onRedo: () => void;
  onDelete: () => void;
  onDuplicate: () => void;
  onAutoLayout: () => void;
  onZoomFit: () => void;
  viewControls?: ViewControls;
  isSaving?: boolean;
  isRunning?: boolean;
  triggerType?: TriggerType;
  triggerInfo?: TriggerInfo;
  onShowHistory?: () => void;
}

function ToolbarButton({
  icon,
  label,
  onClick,
  disabled,
  variant = "default",
  shortcut,
  ...rest
}: {
  icon: string;
  label: string;
  onClick: () => void;
  disabled?: boolean;
  variant?: "default" | "primary" | "danger";
  shortcut?: string;
} & Omit<ButtonHTMLAttributes<HTMLButtonElement>, "onClick" | "disabled">) {
  const tooltipContent = shortcut ? `${label} (${shortcut})` : label;
  return (
    <Tooltip content={tooltipContent} side="bottom">
      <button
        onClick={onClick}
        disabled={disabled}
        aria-label={label}
        {...rest}
        className={cn(
          "flex items-center justify-center w-8 h-8 rounded-lg transition-all duration-150",
          disabled
            ? "opacity-25 cursor-not-allowed"
            : variant === "primary"
              ? "text-electric-indigo hover:bg-electric-indigo/15 active:scale-95"
              : variant === "danger"
                ? "text-red-400/70 hover:bg-red-500/10 hover:text-red-400 active:scale-95"
                : "text-orbflow-text-muted hover:bg-orbflow-controls-btn-hover active:scale-95",
        )}
      >
        <NodeIcon name={icon} className="w-3.5 h-3.5" />
      </button>
    </Tooltip>
  );
}

function ToolbarDivider() {
  return <div className="w-px h-5 mx-0.5 bg-orbflow-border" />;
}

export function BuilderToolbar({
  workflowName,
  onNameChange,
  workflowDescription = "",
  onDescriptionChange,
  onSave,
  onRun,
  onUndo,
  onRedo,
  onDelete,
  onDuplicate,
  onAutoLayout,
  onZoomFit,
  viewControls = {},
  isSaving,
  isRunning,
  triggerType = "manual",
  triggerInfo = {},
  onShowHistory,
}: ToolbarProps) {
  const {
    onAddAnnotation,
    onShowShortcuts,
    onToggleSearch,
    onToggleGrid,
    snapToGrid = false,
  } = viewControls;
  // Derived state from stores -- avoids prop drilling from parent
  const { selectedNodeIds, selectedEdgeIds, nodes, edges } = useCanvasStore();
  const hasSelection = selectedNodeIds.size > 0 || selectedEdgeIds.size > 0;
  const selectionCount = selectedNodeIds.size + selectedEdgeIds.size;
  const nodeCount = nodes.length;
  const edgeCount = edges.length;
  const canUndo = useHistoryStore((s) => s.past.length > 0);
  const canRedo = useHistoryStore((s) => s.future.length > 0);
  const isDirty = useHistoryStore((s) => s.isDirty);
  const [editing, setEditing] = useState(false);
  const [annotationOpen, setAnnotationOpen] = useState(false);
  const annotationRef = useRef<HTMLDivElement>(null);
  // Local editing state -- only used while actively editing, prop is source of truth otherwise
  const [nameValue, setNameValue] = useState("");
  const [editingDesc, setEditingDesc] = useState(false);
  const [descValue, setDescValue] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);
  const descRef = useRef<HTMLInputElement>(null);
  const [copied, setCopied] = useState(false);
  const [cronActive, setCronActive] = useState(false);
  const copiedTimerRef = useRef<ReturnType<typeof setTimeout>>(undefined);

  // Clean up copy timer on unmount
  useEffect(() => {
    return () => {
      if (copiedTimerRef.current) clearTimeout(copiedTimerRef.current);
    };
  }, []);

  useEffect(() => {
    if (editing && inputRef.current) {
      inputRef.current.focus();
      inputRef.current.select();
    }
  }, [editing]);

  useEffect(() => {
    if (editingDesc && descRef.current) {
      descRef.current.focus();
      descRef.current.select();
    }
  }, [editingDesc]);

  // Close annotation dropdown on click outside
  useEffect(() => {
    if (!annotationOpen) return;
    const handler = (e: MouseEvent) => {
      if (annotationRef.current && !annotationRef.current.contains(e.target as HTMLElement)) {
        setAnnotationOpen(false);
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [annotationOpen]);

  const commitName = () => {
    const trimmed = nameValue.trim();
    if (trimmed && trimmed !== workflowName) {
      onNameChange(trimmed);
    }
    setEditing(false);
  };

  const commitDesc = () => {
    const trimmed = descValue.trim();
    if (trimmed !== workflowDescription && onDescriptionChange) {
      onDescriptionChange(trimmed);
    }
    setEditingDesc(false);
  };

  const handleCopyWebhookPath = useCallback(() => {
    const path = triggerInfo.webhookPath || "/webhooks/...";
    navigator.clipboard.writeText(path).then(() => {
      setCopied(true);
      if (copiedTimerRef.current) clearTimeout(copiedTimerRef.current);
      copiedTimerRef.current = setTimeout(() => setCopied(false), 1500);
    });
  }, [triggerInfo.webhookPath]);

  const busy = isSaving || isRunning;

  return (
    <div className="absolute top-4 left-1/2 -translate-x-1/2 z-10 animate-fade-in-up">
      <div role="toolbar" aria-label="Workflow actions" className="flex items-center gap-1 px-2 py-1.5 rounded-2xl backdrop-blur-xl bg-orbflow-glass-bg border border-orbflow-border">
        {/* Workflow name + description */}
        <div className="flex items-center gap-1.5 px-2 min-w-0">
          <div
            className={cn(
              "w-1.5 h-1.5 rounded-full shrink-0 transition-colors duration-300",
              isDirty ? "bg-amber-400" : "bg-neon-cyan animate-pulse-soft"
            )}
            title={isDirty ? "Unsaved changes" : "All changes saved"}
          />
          <div className="flex flex-col min-w-0">
            {editing ? (
              <input
                ref={inputRef}
                value={nameValue}
                onChange={(e) => setNameValue(e.target.value)}
                onBlur={commitName}
                onKeyDown={(e) => {
                  if (e.key === "Enter") commitName();
                  if (e.key === "Escape") {
                    setEditing(false);
                  }
                }}
                className="bg-transparent text-body-lg font-semibold outline-none
                  border-b border-electric-indigo/50 focus-visible:border-electric-indigo
                  min-w-[100px] max-w-[200px] py-0.5 text-orbflow-text-secondary"
              />
            ) : (
              <button
                onClick={() => { setNameValue(workflowName); setEditing(true); }}
                className="text-body-lg font-semibold transition-colors
                  truncate max-w-[200px] py-0.5 text-left text-orbflow-text-muted
                  hover:text-orbflow-text-secondary focus-visible:ring-2 focus-visible:ring-electric-indigo/50
                  focus-visible:outline-none rounded"
                title="Click to rename"
                aria-label={`Rename workflow: ${workflowName}`}
              >
                {workflowName}
              </button>
            )}
            {onDescriptionChange && (
              editingDesc ? (
                <input
                  ref={descRef}
                  value={descValue}
                  onChange={(e) => setDescValue(e.target.value)}
                  onBlur={commitDesc}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") commitDesc();
                    if (e.key === "Escape") {
                      setEditingDesc(false);
                    }
                  }}
                  placeholder="Add a description..."
                  className="bg-transparent text-body-sm outline-none
                    min-w-[100px] max-w-[200px] text-orbflow-text-muted border-b border-orbflow-border-hover
                    focus-visible:border-electric-indigo/50"
                />
              ) : (
                <button
                  onClick={() => { setDescValue(workflowDescription); setEditingDesc(true); }}
                  className="text-body-sm transition-colors
                    truncate max-w-[200px] text-left text-orbflow-text-faint
                    hover:text-orbflow-text-muted focus-visible:ring-2 focus-visible:ring-electric-indigo/50
                    focus-visible:outline-none rounded"
                  title="Click to edit description"
                  aria-label="Edit workflow description"
                >
                  {workflowDescription || "Add description..."}
                </button>
              )
            )}
          </div>
        </div>

        <ToolbarDivider />

        {/* Undo / Redo */}
        <ToolbarButton
          icon="undo"
          label="Undo"
          onClick={onUndo}
          disabled={!canUndo}
          shortcut="Ctrl+Z"
        />
        <ToolbarButton
          icon="redo"
          label="Redo"
          onClick={onRedo}
          disabled={!canRedo}
          shortcut="Ctrl+Shift+Z"
        />

        <ToolbarDivider />

        {/* Selection actions */}
        <ToolbarButton
          icon="copy"
          label="Duplicate"
          onClick={onDuplicate}
          disabled={!hasSelection}
          shortcut="Ctrl+D"
        />
        <ToolbarButton
          icon="trash"
          label="Delete"
          onClick={onDelete}
          disabled={!hasSelection}
          variant="danger"
          shortcut="Del"
        />

        <ToolbarDivider />

        {/* Layout */}
        <ToolbarButton
          icon="auto-layout"
          label="Auto Layout"
          onClick={onAutoLayout}
          disabled={nodeCount === 0}
        />
        <ToolbarButton
          icon="zoom-fit"
          label="Zoom to Fit"
          onClick={onZoomFit}
          disabled={nodeCount === 0}
        />

        {/* Annotations */}
        {onAddAnnotation && (
          <>
            <ToolbarDivider />
            <div className="relative" ref={annotationRef}>
              <ToolbarButton
                icon="message-square"
                label="Add Annotation"
                onClick={() => setAnnotationOpen(!annotationOpen)}
                aria-expanded={annotationOpen}
                aria-haspopup="menu"
              />
              {annotationOpen && (
                <div
                  role="menu"
                  aria-label="Annotation types"
                  className="absolute top-full left-1/2 -translate-x-1/2 mt-2 w-40
                    rounded-xl border border-orbflow-border bg-orbflow-surface shadow-xl py-1 z-50
                    animate-scale-in"
                  onKeyDown={(e) => {
                    if (e.key === "Escape") { e.stopPropagation(); setAnnotationOpen(false); return; }
                    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
                      e.preventDefault();
                      const items = e.currentTarget.querySelectorAll<HTMLButtonElement>("[role='menuitem']");
                      const active = document.activeElement as HTMLElement;
                      const idx = Array.from(items).indexOf(active as HTMLButtonElement);
                      const next = e.key === "ArrowDown"
                        ? (idx + 1) % items.length
                        : (idx - 1 + items.length) % items.length;
                      items[next]?.focus();
                    }
                  }}
                  ref={(el) => { if (el) { const first = el.querySelector<HTMLButtonElement>("[role='menuitem']"); first?.focus(); } }}
                >
                  <button
                    role="menuitem"
                    tabIndex={-1}
                    onClick={() => { onAddAnnotation("sticky_note"); setAnnotationOpen(false); }}
                    className="flex items-center gap-2 w-full px-3 py-2 text-body font-medium
                      text-orbflow-text-secondary hover:bg-orbflow-surface-hover focus-visible:bg-orbflow-surface-hover
                      transition-colors outline-none animate-fade-in stagger-1"
                  >
                    <NodeIcon name="message-square" className="w-3.5 h-3.5 text-orbflow-text-muted" />
                    Sticky Note
                  </button>
                  <button
                    role="menuitem"
                    tabIndex={-1}
                    onClick={() => { onAddAnnotation("text"); setAnnotationOpen(false); }}
                    className="flex items-center gap-2 w-full px-3 py-2 text-body font-medium
                      text-orbflow-text-secondary hover:bg-orbflow-surface-hover focus-visible:bg-orbflow-surface-hover
                      transition-colors outline-none animate-fade-in stagger-2"
                  >
                    <NodeIcon name="type" className="w-3.5 h-3.5 text-orbflow-text-muted" />
                    Text Label
                  </button>
                </div>
              )}
            </div>
          </>
        )}

        {(onToggleSearch || onToggleGrid || onShowShortcuts) && <ToolbarDivider />}

        {onToggleSearch && (
          <ToolbarButton
            icon="search"
            label="Search Nodes"
            onClick={onToggleSearch}
            shortcut="Ctrl+F"
          />
        )}
        {onToggleGrid && (
          <Tooltip content={snapToGrid ? "Disable snap to grid (Ctrl+G)" : "Enable snap to grid (Ctrl+G)"} side="bottom">
            <button
              onClick={onToggleGrid}
              aria-label="Toggle snap to grid"
              className={cn(
                "flex items-center justify-center w-8 h-8 rounded-lg transition-all duration-150",
                snapToGrid
                  ? "text-electric-indigo bg-electric-indigo/15 active:scale-95"
                  : "text-orbflow-text-muted hover:bg-orbflow-controls-btn-hover active:scale-95",
              )}
            >
              <NodeIcon name="grid" className="w-3.5 h-3.5" />
            </button>
          </Tooltip>
        )}
        {onShowShortcuts && (
          <ToolbarButton
            icon="help-circle"
            label="Keyboard Shortcuts"
            onClick={onShowShortcuts}
            shortcut="?"
          />
        )}

        <ToolbarDivider />

        {/* Selection count / Stats */}
        {selectionCount > 1 ? (
          <span className="text-body-sm font-medium px-2 py-0.5 rounded-md bg-electric-indigo/10 text-electric-indigo whitespace-nowrap">
            {selectionCount} selected
          </span>
        ) : (
          <span className="text-body-sm font-mono px-1.5 whitespace-nowrap text-orbflow-text-faint">
            {nodeCount}n · {edgeCount}e
          </span>
        )}

        {/* Version history */}
        {onShowHistory && (
          <>
            <ToolbarDivider />
            <ToolbarButton
              icon="clock"
              label="Version History"
              onClick={onShowHistory}
            />
          </>
        )}

        <ToolbarDivider />

        {/* Save */}
        <button
          onClick={onSave}
          disabled={busy}
          aria-label={isSaving ? "Saving workflow" : "Save workflow"}
          className={cn(
            "flex items-center gap-1.5 rounded-lg px-3 py-1.5 text-body font-medium transition-all duration-150 border border-orbflow-border bg-orbflow-add-btn-bg text-orbflow-text-secondary",
            busy ? "opacity-60 cursor-not-allowed" : "hover:bg-orbflow-controls-btn-hover active:scale-[0.97]"
          )}
        >
          <NodeIcon name={isSaving ? "loader" : "save"} className={cn("w-3.5 h-3.5", isSaving && "animate-spin")} />
          {isSaving ? "Saving\u2026" : "Save"}
        </button>

        {/* Trigger-aware run section */}
        {onRun && (
          <TriggerRunSection
            triggerType={triggerType}
            triggerInfo={triggerInfo}
            onRun={onRun}
            busy={busy}
            isRunning={isRunning}
            copied={copied}
            onCopy={handleCopyWebhookPath}
            cronActive={cronActive}
            onToggleCron={() => setCronActive((v) => !v)}
          />
        )}
      </div>
    </div>
  );
}

// -- Cron expression to human-readable string -----------

function cronToHuman(cron: string): string {
  const parts = cron.trim().split(/\s+/);
  if (parts.length < 5) return cron;
  const [minute, hour, dayOfMonth, month, dayOfWeek] = parts;

  // Every N minutes: */N * * * *
  if (minute.startsWith("*/") && hour === "*" && dayOfMonth === "*" && month === "*" && dayOfWeek === "*") {
    const n = minute.slice(2);
    return n === "1" ? "Every minute" : `Every ${n} min`;
  }
  // Every hour at :MM
  if (!minute.includes("*") && !minute.includes("/") && hour === "*" && dayOfMonth === "*" && month === "*" && dayOfWeek === "*") {
    return `Hourly at :${minute.padStart(2, "0")}`;
  }
  // Every N hours
  if (hour.startsWith("*/") && dayOfMonth === "*" && month === "*" && dayOfWeek === "*") {
    const n = hour.slice(2);
    return n === "1" ? "Every hour" : `Every ${n} hours`;
  }
  // Daily at HH:MM
  if (!minute.includes("*") && !hour.includes("*") && dayOfMonth === "*" && month === "*" && dayOfWeek === "*") {
    return `Daily at ${hour.padStart(2, "0")}:${minute.padStart(2, "0")}`;
  }
  return cron;
}

// -- Trigger-aware run section --------------------------

function TriggerRunSection({
  triggerType,
  triggerInfo,
  onRun,
  busy,
  isRunning,
  copied,
  onCopy,
  cronActive,
  onToggleCron,
}: {
  triggerType: TriggerType;
  triggerInfo: TriggerInfo;
  onRun: () => void;
  busy?: boolean;
  isRunning?: boolean;
  copied: boolean;
  onCopy: () => void;
  cronActive: boolean;
  onToggleCron: () => void;
}) {
  // Manual trigger: standard Run button
  if (triggerType === "manual") {
    return (
      <button
        onClick={onRun}
        disabled={busy}
        className={cn(
          "flex items-center gap-1.5 rounded-lg bg-electric-indigo px-3.5 py-1.5 text-body font-semibold text-white shadow-lg shadow-indigo-500/20 transition-all duration-150",
          busy ? "opacity-60 cursor-not-allowed" : "hover:shadow-indigo-500/30 hover:brightness-110 active:scale-[0.97]"
        )}
      >
        <NodeIcon name={isRunning ? "loader" : "play"} className={cn("w-3.5 h-3.5", isRunning && "animate-spin")} />
        {isRunning ? "Running\u2026" : "Run"}
      </button>
    );
  }

  // Webhook trigger: path badge + copy + test
  if (triggerType === "webhook") {
    return (
      <div className="flex items-center gap-1">
        <div
          className="flex items-center gap-1.5 rounded-lg px-2.5 py-1.5 border border-purple-500/20 bg-purple-500/[0.06]"
          title={triggerInfo.webhookPath || "/webhooks/..."}
        >
          <NodeIcon name="link" className="w-3 h-3 text-purple-400/70 shrink-0" />
          <span className="text-body-sm font-mono text-purple-300/80 truncate max-w-[120px]">
            {triggerInfo.webhookPath || "/webhooks/..."}
          </span>
          <button
            onClick={onCopy}
            className="flex items-center justify-center w-5 h-5 rounded hover:bg-orbflow-controls-btn-hover transition-colors shrink-0"
            title={copied ? "Copied!" : "Copy webhook path"}
            aria-label="Copy webhook path"
          >
            <NodeIcon name={copied ? "check" : "clipboard"} className={cn("w-3 h-3", copied ? "text-neon-cyan" : "text-orbflow-text-muted")} />
          </button>
        </div>
        <TestRunButton onRun={onRun} busy={busy} isRunning={isRunning} />
      </div>
    );
  }

  // Cron trigger: schedule badge + active toggle + test
  if (triggerType === "cron") {
    const humanCron = cronToHuman(triggerInfo.cronExpression || "* * * * *");
    return (
      <div className="flex items-center gap-1">
        <div
          className="flex items-center gap-1.5 rounded-lg px-2.5 py-1.5 border border-amber-500/20 bg-amber-500/[0.06]"
          title={triggerInfo.cronExpression || "* * * * *"}
        >
          <NodeIcon name="clock" className="w-3 h-3 text-amber-400/70 shrink-0" />
          <span className="text-body-sm font-medium text-amber-300/80 whitespace-nowrap">
            {humanCron}
          </span>
        </div>
        <button
          onClick={onToggleCron}
          className={cn(
            "flex items-center gap-1.5 rounded-lg px-2.5 py-1.5 text-body-sm font-semibold transition-all duration-150 active:scale-[0.97]",
            cronActive
              ? "bg-neon-cyan/15 border border-neon-cyan/25 text-neon-cyan hover:bg-neon-cyan/20"
              : "bg-orbflow-add-btn-bg border border-orbflow-border text-orbflow-text-muted hover:bg-orbflow-controls-btn-hover"
          )}
          title={cronActive ? "Deactivate schedule" : "Activate schedule"}
        >
          <div className={cn("w-1.5 h-1.5 rounded-full", cronActive ? "bg-neon-cyan animate-pulse-soft" : "bg-orbflow-text-ghost")} />
          {cronActive ? "Active" : "Inactive"}
        </button>
        <TestRunButton onRun={onRun} busy={busy} isRunning={isRunning} />
      </div>
    );
  }

  // Event trigger: event name badge + test
  if (triggerType === "event") {
    return (
      <div className="flex items-center gap-1">
        <div
          className="flex items-center gap-1.5 rounded-lg px-2.5 py-1.5 border border-red-500/20 bg-red-500/[0.06]"
          title={triggerInfo.eventName || "event"}
        >
          <NodeIcon name="radio" className="w-3 h-3 text-red-400/70 shrink-0" />
          <span className="text-body-sm font-mono text-red-300/80 truncate max-w-[120px]">
            {triggerInfo.eventName || "event"}
          </span>
        </div>
        <TestRunButton onRun={onRun} busy={busy} isRunning={isRunning} />
      </div>
    );
  }

  // Fallback: standard run button
  return (
    <button
      onClick={onRun}
      disabled={busy}
      className={cn(
        "flex items-center gap-1.5 rounded-lg bg-electric-indigo px-3.5 py-1.5 text-body font-semibold text-white shadow-lg shadow-indigo-500/20 transition-all duration-150",
        busy ? "opacity-60 cursor-not-allowed" : "hover:shadow-indigo-500/30 hover:brightness-110 active:scale-[0.97]"
      )}
    >
      <NodeIcon name={isRunning ? "loader" : "play"} className={cn("w-3.5 h-3.5", isRunning && "animate-spin")} />
      {isRunning ? "Running\u2026" : "Run"}
    </button>
  );
}

// -- Small "Test Run" button for non-manual triggers ----

function TestRunButton({
  onRun,
  busy,
  isRunning,
}: {
  onRun: () => void;
  busy?: boolean;
  isRunning?: boolean;
}) {
  return (
    <button
      onClick={onRun}
      disabled={busy}
      className={cn(
        "flex items-center gap-1 rounded-lg bg-electric-indigo/80 px-2.5 py-1.5 text-body-sm font-semibold text-white shadow-md shadow-indigo-500/15 transition-all duration-150",
        busy ? "opacity-60 cursor-not-allowed" : "hover:bg-electric-indigo hover:shadow-indigo-500/25 active:scale-[0.97]"
      )}
      title="Test run this workflow"
      aria-label="Test run this workflow"
    >
      <NodeIcon name={isRunning ? "loader" : "play"} className={cn("w-3 h-3", isRunning && "animate-spin")} />
      {isRunning ? "Running\u2026" : "Test"}
    </button>
  );
}
