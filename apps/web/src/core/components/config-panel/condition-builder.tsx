"use client";

import { useMemo, useCallback } from "react";
import type { ConditionGroup, ConditionRule } from "../../types/schema";
import { isConditionGroup } from "../../types/schema";
import type { UpstreamOutput } from "../../utils/upstream";
import { ConditionRow } from "./condition-row";
import { NodeIcon } from "../icons";
import { cn } from "../../utils/cn";

interface ConditionBuilderProps {
  condition: ConditionGroup;
  upstream: UpstreamOutput[];
  onUpdate: (updated: ConditionGroup) => void;
  depth?: number;
}

/** Flatten upstream fields into selector options */
function buildFieldOptions(upstream: UpstreamOutput[]) {
  const options: { label: string; celPath: string; type: string }[] = [];

  function walk(
    fields: { key: string; type: string; children?: typeof fields }[],
    nodeId: string,
    nodeName: string,
    celPrefix: string,
    labelPrefix: string,
  ) {
    for (const f of fields) {
      const celPath = `${celPrefix}.${f.key}`;
      const label = labelPrefix ? `${nodeName} -> ${labelPrefix}.${f.key}` : `${nodeName} -> ${f.key}`;
      options.push({ label, celPath, type: f.type });
      if (f.children && f.children.length > 0) {
        walk(f.children, nodeId, nodeName, celPath, labelPrefix ? `${labelPrefix}.${f.key}` : f.key);
      }
    }
  }

  for (const node of upstream) {
    walk(node.fields, node.nodeId, node.nodeName, `nodes["${node.nodeId}"]`, "");
  }

  // Context
  options.push(
    { label: "vars", celPath: "vars", type: "object" },
    { label: "trigger", celPath: "trigger", type: "object" },
  );

  return options;
}

export function ConditionBuilder({
  condition,
  upstream,
  onUpdate,
  depth = 0,
}: ConditionBuilderProps) {
  const fieldOptions = useMemo(() => buildFieldOptions(upstream), [upstream]);

  const handleAddRule = useCallback(() => {
    const rule: ConditionRule = {
      id: `rule_${Date.now()}`,
      field: "",
      operator: "==",
      value: "",
    };
    onUpdate({ ...condition, rules: [...condition.rules, rule] });
  }, [condition, onUpdate]);

  const handleAddGroup = useCallback(() => {
    const nested: ConditionGroup = {
      id: `group_${Date.now()}`,
      logic: condition.logic === "and" ? "or" : "and",
      rules: [],
    };
    onUpdate({ ...condition, rules: [...condition.rules, nested] });
  }, [condition, onUpdate]);

  const handleUpdateRule = useCallback((index: number, updated: ConditionRule | ConditionGroup) => {
    const newRules = condition.rules.map((r, i) => (i === index ? updated : r));
    onUpdate({ ...condition, rules: newRules });
  }, [condition, onUpdate]);

  const handleDeleteRule = useCallback((index: number) => {
    onUpdate({ ...condition, rules: condition.rules.filter((_, i) => i !== index) });
  }, [condition, onUpdate]);

  const handleToggleLogic = useCallback(() => {
    onUpdate({ ...condition, logic: condition.logic === "and" ? "or" : "and" });
  }, [condition, onUpdate]);

  const isAnd = condition.logic === "and";
  const maxDepth = 3;

  return (
    <div
      className={cn(
        "relative rounded-lg border transition-colors duration-200",
        depth === 0
          ? "border-orbflow-border bg-orbflow-bg/50 p-3"
          : "border-orbflow-border/60 bg-orbflow-surface/30 p-2.5 ml-3",
      )}
    >
      {/* Logic toggle pill */}
      <div className="flex items-center gap-2 mb-2.5">
        <button
          onClick={handleToggleLogic}
          className={cn(
            "inline-flex items-center gap-1 px-2.5 py-1 rounded-full text-body-sm font-bold uppercase tracking-wider",
            "transition-all duration-200 cursor-pointer",
            "focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none",
            isAnd
              ? "bg-electric-indigo/15 text-electric-indigo border border-electric-indigo/25 hover:bg-electric-indigo/20"
              : "bg-amber-400/15 text-amber-400 border border-amber-400/25 hover:bg-amber-400/20",
          )}
          title={`Switch to ${isAnd ? "OR" : "AND"} logic`}
          aria-label={`Current logic: ${condition.logic}. Click to toggle.`}
        >
          {isAnd ? "AND" : "OR"}
          <NodeIcon name="repeat" className="w-2.5 h-2.5 opacity-50" />
        </button>

        <span className="text-caption text-orbflow-text-ghost">
          {isAnd ? "All conditions must match" : "Any condition can match"}
        </span>
      </div>

      {/* Rules */}
      <div className="space-y-2">
        {condition.rules.map((rule, index) => {
          // Separator between rules
          const showSeparator = index > 0;

          return (
            <div key={rule.id}>
              {showSeparator && (
                <div className="flex items-center gap-2 py-1">
                  <div className="flex-1 h-px bg-orbflow-border/50" />
                  <span className={cn(
                    "text-micro font-bold uppercase tracking-wider px-1.5",
                    isAnd ? "text-electric-indigo/40" : "text-amber-400/40",
                  )}>
                    {condition.logic}
                  </span>
                  <div className="flex-1 h-px bg-orbflow-border/50" />
                </div>
              )}

              {isConditionGroup(rule) ? (
                <ConditionBuilder
                  condition={rule}
                  upstream={upstream}
                  onUpdate={(updated) => handleUpdateRule(index, updated)}
                  depth={depth + 1}
                />
              ) : (
                <ConditionRow
                  rule={rule}
                  fieldOptions={fieldOptions}
                  onChange={(updated) => handleUpdateRule(index, updated)}
                  onDelete={() => handleDeleteRule(index)}
                />
              )}
            </div>
          );
        })}
      </div>

      {/* Empty state */}
      {condition.rules.length === 0 && (
        <div className="text-center py-4">
          <p className="text-body-sm text-orbflow-text-ghost mb-2">No conditions yet</p>
          <p className="text-caption text-orbflow-text-ghost">Add a rule to filter when this edge activates</p>
        </div>
      )}

      {/* Add buttons */}
      <div className="flex items-center gap-2 mt-2.5 pt-2 border-t border-orbflow-border/40">
        <button
          onClick={handleAddRule}
          className="inline-flex items-center gap-1 px-2 py-1 rounded-md text-body-sm text-orbflow-text-muted
            hover:bg-orbflow-surface-hover hover:text-orbflow-text-secondary transition-colors cursor-pointer
            focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none"
        >
          <NodeIcon name="plus" className="w-3 h-3" />
          Add rule
        </button>

        {depth < maxDepth && (
          <button
            onClick={handleAddGroup}
            className="inline-flex items-center gap-1 px-2 py-1 rounded-md text-body-sm text-orbflow-text-muted
              hover:bg-orbflow-surface-hover hover:text-orbflow-text-secondary transition-colors cursor-pointer
              focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none"
          >
            <NodeIcon name="git-branch" className="w-3 h-3" />
            Add group
          </button>
        )}
      </div>
    </div>
  );
}
