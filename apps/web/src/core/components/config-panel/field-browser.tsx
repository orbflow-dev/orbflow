"use client";

import { useState, useMemo } from "react";
import type { FieldSchema } from "../../types/schema";
import type { UpstreamOutput } from "../../utils/upstream";
import { NodeIcon } from "../icons";

const TYPE_COLORS: Record<string, string> = {
  string: "bg-emerald-400",
  number: "bg-amber-400",
  boolean: "bg-purple-400",
  object: "bg-blue-400",
  array: "bg-pink-400",
};

interface FieldBrowserProps {
  upstream: UpstreamOutput[];
  selectedPath?: string;
  onSelect: (nodeId: string, path: string, celPath: string) => void;
}

export function FieldBrowser({
  upstream,
  selectedPath,
  onSelect,
}: FieldBrowserProps) {
  const [searchFilter, setSearchFilter] = useState("");

  const filteredUpstream = useMemo(() => {
    if (!searchFilter.trim()) return upstream;
    const q = searchFilter.toLowerCase();
    return upstream
      .map((node) => ({
        ...node,
        fields: filterFields(node.fields, q),
      }))
      .filter((node) => node.fields.length > 0);
  }, [upstream, searchFilter]);

  return (
    <div className="space-y-1">
      {/* Search filter */}
      {upstream.length > 0 && (
        <div className="relative mb-1.5">
          <NodeIcon name="search" className="absolute left-2 top-1/2 -translate-y-1/2 w-3 h-3 text-orbflow-text-ghost pointer-events-none" />
          <input
            type="text"
            value={searchFilter}
            onChange={(e) => setSearchFilter(e.target.value)}
            placeholder="Filter fields..."
            className="w-full rounded-md pl-7 pr-2 py-1.5 text-body-sm
              border border-orbflow-border bg-orbflow-add-btn-bg text-orbflow-text-secondary
              placeholder:text-orbflow-text-ghost focus:outline-none focus:border-electric-indigo/30
              focus-visible:ring-2 focus-visible:ring-electric-indigo/50 transition-colors"
          />
        </div>
      )}

      <div className="max-h-64 overflow-y-auto custom-scrollbar space-y-1">
        {filteredUpstream.map((node) => (
          <NodeBranch
            key={node.nodeId}
            node={node}
            selectedPath={selectedPath}
            onSelect={onSelect}
          />
        ))}

        {/* Context variables always available */}
        {!searchFilter.trim() && (
          <div className="mt-2 pt-2 border-t border-orbflow-border">
            <div className="text-caption font-mono text-orbflow-text-ghost uppercase tracking-wider px-2 mb-1">
              Context
            </div>
            <ContextItem
              label="vars"
              description="Workflow input variables"
              prefix="vars"
              selected={selectedPath?.startsWith("vars") || false}
              onSelect={(path) => onSelect("__context__", path, path)}
            />
            <ContextItem
              label="trigger"
              description="Trigger data"
              prefix="trigger"
              selected={selectedPath?.startsWith("trigger") || false}
              onSelect={(path) => onSelect("__context__", path, path)}
            />
          </div>
        )}

        {upstream.length === 0 && (
          <div className="text-xs text-orbflow-text-ghost text-center py-4">
            No upstream nodes connected
          </div>
        )}
      </div>
    </div>
  );
}

/** Recursively filter fields by search query on key name */
function filterFields(fields: FieldSchema[], query: string): FieldSchema[] {
  const result: FieldSchema[] = [];
  for (const field of fields) {
    const keyMatch = field.key.toLowerCase().includes(query);
    const filteredChildren = field.children ? filterFields(field.children, query) : [];
    if (keyMatch || filteredChildren.length > 0) {
      result.push({ ...field, children: keyMatch ? field.children : filteredChildren });
    }
  }
  return result;
}

function NodeBranch({
  node,
  selectedPath,
  onSelect,
}: {
  node: UpstreamOutput;
  selectedPath?: string;
  onSelect: (nodeId: string, path: string, celPath: string) => void;
}) {
  const [open, setOpen] = useState(true);

  return (
    <div>
      <button
        onClick={() => setOpen(!open)}
        className="w-full flex items-center gap-2 px-2 py-1.5 rounded-lg text-left hover:bg-orbflow-surface-hover transition-colors
          focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none"
      >
        <NodeIcon
          name="chevron-down"
          className={`w-3 h-3 text-orbflow-text-faint shrink-0 transition-transform duration-200 ${open ? "" : "-rotate-90"}`}
        />
        <span className="text-xs font-medium text-orbflow-text-secondary truncate">
          {node.nodeName}
        </span>
        <span className="text-caption font-mono text-orbflow-text-ghost ml-auto truncate">
          {node.pluginRef}
        </span>
        {node.pluginRef.startsWith("builtin:trigger-") && (
          <span className="text-[9px] font-bold uppercase tracking-wider px-1 py-0.5 rounded
            bg-emerald-500/10 text-emerald-400/70 border border-emerald-500/15 ml-1 shrink-0">
            trigger output
          </span>
        )}
      </button>
      {open && (
        <div className="ml-3 border-l border-orbflow-border pl-2">
          {node.fields.map((field) => (
            <FieldLeaf
              key={field.key}
              field={field}
              nodeId={node.nodeId}
              parentPath=""
              celPrefix={`nodes["${node.nodeId}"]`}
              selectedPath={selectedPath}
              onSelect={onSelect}
            />
          ))}
        </div>
      )}
    </div>
  );
}

function FieldLeaf({
  field,
  nodeId,
  parentPath,
  celPrefix,
  selectedPath,
  onSelect,
}: {
  field: FieldSchema;
  nodeId: string;
  parentPath: string;
  celPrefix: string;
  selectedPath?: string;
  onSelect: (nodeId: string, path: string, celPath: string) => void;
}) {
  const [open, setOpen] = useState(false);
  const path = parentPath ? `${parentPath}.${field.key}` : field.key;
  const celPath = `${celPrefix}.${field.key}`;
  const isSelected = selectedPath === celPath;
  const hasChildren = field.children && field.children.length > 0;

  const textTypeColors: Record<string, string> = {
    string: "text-emerald-400",
    number: "text-amber-400",
    boolean: "text-purple-400",
    object: "text-blue-400",
    array: "text-pink-400",
  };

  const handleDragStart = (e: React.DragEvent) => {
    e.dataTransfer.setData(
      "application/orbflow-field",
      JSON.stringify({ nodeId, path, celPath })
    );
    e.dataTransfer.effectAllowed = "copy";
    // Rich drag preview text
    e.dataTransfer.setData("text/plain", celPath);
  };

  return (
    <div>
      <button
        draggable
        onDragStart={handleDragStart}
        onClick={() => {
          if (hasChildren) {
            setOpen(!open);
          }
          onSelect(nodeId, path, celPath);
        }}
        className={`w-full flex items-center gap-2 px-2 py-1 rounded-md text-left transition-all text-xs cursor-grab active:cursor-grabbing
          focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none ${
            isSelected
              ? "bg-electric-indigo/15 text-electric-indigo border border-electric-indigo/20"
              : "hover:bg-orbflow-surface-hover border border-transparent"
          }`}
      >
        {hasChildren ? (
          <NodeIcon
            name="chevron-down"
            className={`w-2.5 h-2.5 text-orbflow-text-ghost shrink-0 transition-transform duration-200 ${open ? "" : "-rotate-90"}`}
          />
        ) : (
          <span className="w-2.5 shrink-0" />
        )}
        {/* Type color dot */}
        <span className={`w-1.5 h-1.5 rounded-full shrink-0 ${TYPE_COLORS[field.type] || "bg-orbflow-text-ghost"}`} />
        <span className={`font-mono ${isSelected ? "text-electric-indigo" : "text-orbflow-text-secondary"}`}>
          {field.key}
        </span>
        <span className={`text-caption font-mono ml-auto ${textTypeColors[field.type] || "text-orbflow-text-ghost"}`}>
          {field.type}
        </span>
        {field.dynamic && (
          <span className="text-[9px] text-amber-400/50 ml-0.5 shrink-0"
            title="Dynamic -- actual shape known after execution">
            ~
          </span>
        )}
        {field.isBinary && (
          <span className="text-[9px] font-bold text-rose-400/60 ml-0.5 shrink-0 px-1 py-px rounded bg-rose-500/10 border border-rose-500/15"
            title="Binary data -- cannot be used as text input">
            BIN
          </span>
        )}
      </button>
      {/* Hint: dynamic field without runtime data yet */}
      {field.dynamic && !hasChildren && !field.isBinary && (
        <div className="ml-5 mt-0.5 mb-0.5">
          <p className="text-[10px] text-amber-400/40 italic flex items-center gap-1">
            <NodeIcon name="zap" className="w-2.5 h-2.5" />
            Run this node to see output structure
          </p>
        </div>
      )}
      {open && hasChildren && (
        <div className="ml-3 border-l border-orbflow-border pl-2">
          {(field.children ?? []).map((child) => (
            <FieldLeaf
              key={child.key}
              field={child}
              nodeId={nodeId}
              parentPath={path}
              celPrefix={celPath}
              selectedPath={selectedPath}
              onSelect={onSelect}
            />
          ))}
        </div>
      )}
    </div>
  );
}

function ContextItem({
  label,
  description,
  prefix,
  selected,
  onSelect,
}: {
  label: string;
  description: string;
  prefix: string;
  selected: boolean;
  onSelect: (path: string) => void;
}) {
  return (
    <button
      onClick={() => onSelect(prefix)}
      className={`w-full flex items-center gap-2 px-2 py-1.5 rounded-md text-left text-xs transition-colors
        focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none
        ${selected ? "bg-electric-indigo/10 text-electric-indigo" : "hover:bg-orbflow-surface-hover text-orbflow-text-muted"}`}
    >
      <span className="font-mono">{label}</span>
      <span className="text-caption text-orbflow-text-ghost ml-auto">{description}</span>
    </button>
  );
}
