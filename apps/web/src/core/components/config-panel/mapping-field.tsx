"use client";

import { useState, useCallback } from "react";
import type { FieldSchema, FieldMapping } from "../../types/schema";
import type { UpstreamOutput } from "../../utils/upstream";
import { FieldBrowser } from "./field-browser";
import { CelExpressionEditor } from "./cel-expression-editor";
import { getTypeColor, getTypeLabel, NodeIcon } from "../icons";
import { CredentialSelector } from "@/components/credential-manager/credential-selector";
import { useToastStore } from "@orbflow/core/stores";

interface WiredInfo {
  sourceNodeId: string;
  sourceNodeLabel: string;
  sourceField: string;
}

interface MappingFieldProps {
  field: FieldSchema;
  mapping?: FieldMapping;
  upstream: UpstreamOutput[];
  onChange: (mapping: FieldMapping) => void;
  wiredFrom?: WiredInfo;
  onFocus?: (fieldKey: string) => void;
}

export function MappingField({
  field,
  mapping,
  upstream,
  onChange,
  wiredFrom,
  onFocus,
}: MappingFieldProps) {
  const [showBrowser, setShowBrowser] = useState(false);
  const [overrideWired, setOverrideWired] = useState(false);
  const [dropHighlight, setDropHighlight] = useState(false);
  const mode = mapping?.mode || "static";

  const isWired = !!wiredFrom && !overrideWired;

  const isUsingDefault =
    field.default !== undefined &&
    mode === "static" &&
    !isWired &&
    (!mapping || mapping.staticValue === undefined);

  const toggleMode = useCallback(() => {
    const newMode = mode === "static" ? "expression" : "static";
    if (newMode === "expression") setShowBrowser(true);
    onChange({
      targetKey: field.key,
      mode: newMode,
      staticValue: mapping?.staticValue,
      sourceNodeId: mapping?.sourceNodeId,
      sourcePath: mapping?.sourcePath,
      celExpression: mapping?.celExpression,
    });
  }, [mode, field.key, mapping, onChange]);

  const handleStaticChange = useCallback(
    (value: string) => {
      onChange({ targetKey: field.key, mode: "static", staticValue: value });
    },
    [field.key, onChange]
  );

  const handleFieldSelect = useCallback(
    (nodeId: string, path: string, celPath: string) => {
      // If currently inside a function call, insert the field as the argument
      const current = mapping?.celExpression ?? "";
      let finalExpression = celPath;
      if (current) {
        let depth = 0;
        let lastOpenIdx = -1;
        for (let i = current.length - 1; i >= 0; i--) {
          if (current[i] === ")") depth++;
          else if (current[i] === "(") {
            if (depth > 0) depth--;
            else { lastOpenIdx = i; break; }
          }
        }
        if (lastOpenIdx >= 0) {
          finalExpression = current.slice(0, lastOpenIdx + 1) + celPath;
        }
      }

      onChange({
        targetKey: field.key,
        mode: "expression",
        sourceNodeId: nodeId === "__context__" ? undefined : nodeId,
        sourcePath: path,
        celExpression: finalExpression,
      });
      setShowBrowser(false);
    },
    [field.key, mapping?.celExpression, onChange]
  );

  const handleDragOver = useCallback((e: React.DragEvent) => {
    if (e.dataTransfer.types.includes("application/orbflow-field")) {
      e.preventDefault();
      e.dataTransfer.dropEffect = "copy";
      setDropHighlight(true);
    }
  }, []);

  const handleDragLeave = useCallback(() => {
    setDropHighlight(false);
  }, []);

  const handleDrop = useCallback(
    (e: React.DragEvent) => {
      e.preventDefault();
      setDropHighlight(false);
      const raw = e.dataTransfer.getData("application/orbflow-field");
      if (!raw) return;
      try {
        const { nodeId, path, celPath } = JSON.parse(raw) as {
          nodeId: string;
          path: string;
          celPath: string;
        };

        // Warn if dropping a binary field onto a non-object input
        const sourceNode = upstream.find((n) => n.nodeId === nodeId);
        if (sourceNode && field.type !== "object") {
          const findField = (
            fields: { key: string; isBinary?: boolean; children?: typeof fields }[],
            parts: string[],
          ): { isBinary?: boolean } | undefined => {
            const [first, ...rest] = parts;
            const f = fields.find((fl) => fl.key === first);
            if (!f) return undefined;
            if (rest.length === 0) return f;
            return f.children ? findField(f.children, rest) : undefined;
          };
          const dropped = findField(sourceNode.fields, path.split("."));
          if (dropped?.isBinary) {
            useToastStore.getState().warning(
              "Binary field",
              "This field contains binary data and may not render correctly as text.",
            );
          }
        }

        // If the current expression has an unclosed function call (ends with `(`
        // or has partial text inside parens), insert the dropped field as the
        // argument instead of replacing the whole expression.
        const current = mapping?.celExpression ?? "";
        let finalExpression = celPath;
        if (current) {
          // Find last unmatched open paren
          let depth = 0;
          let lastOpenIdx = -1;
          for (let i = current.length - 1; i >= 0; i--) {
            if (current[i] === ")") depth++;
            else if (current[i] === "(") {
              if (depth > 0) depth--;
              else { lastOpenIdx = i; break; }
            }
          }
          if (lastOpenIdx >= 0) {
            // Inside a function call -- insert the dropped field after the open paren
            const prefix = current.slice(0, lastOpenIdx + 1);
            finalExpression = prefix + celPath;
          }
        }

        onChange({
          targetKey: field.key,
          mode: "expression",
          sourceNodeId: nodeId === "__context__" ? undefined : nodeId,
          sourcePath: path,
          celExpression: finalExpression,
        });
      } catch (err) {
        console.error("[orbflow] Failed to parse drag-drop field data:", err);
      }
    },
    [field.key, field.type, upstream, mapping?.celExpression, onChange],
  );

  return (
    <div
      className={`transition-all duration-200 ${
        dropHighlight
          ? "ring-1 ring-electric-indigo/40 bg-electric-indigo/[0.04] rounded-xl p-3 -m-3"
          : ""
      }`}
      onDragOver={handleDragOver}
      onDragLeave={handleDragLeave}
      onDrop={handleDrop}
    >
      {/* Label row with Fixed/Expression toggle */}
      <div className="flex items-center justify-between mb-2">
        <label className="text-body-lg font-medium text-orbflow-text-muted flex items-center gap-1.5">
          {field.label}
          {field.required && (
            <span className="text-rose-400/70 text-body-sm">*</span>
          )}
          {isUsingDefault && (
            <span className="text-micro font-medium text-orbflow-text-ghost bg-orbflow-add-btn-bg border border-orbflow-border px-1.5 py-px rounded ml-1">
              default
            </span>
          )}
        </label>

        {!isWired && (
          <div className="inline-flex rounded-md border border-orbflow-border bg-orbflow-bg overflow-hidden">
            <button
              onClick={mode === "expression" ? toggleMode : undefined}
              aria-pressed={mode === "static"}
              className={`px-2.5 py-1 text-body-sm font-medium transition-all
                focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none ${
                mode === "static"
                  ? "bg-orbflow-add-btn-bg text-orbflow-text-secondary"
                  : "text-orbflow-text-faint hover:text-orbflow-text-muted"
              }`}
            >
              Fixed
            </button>
            <button
              onClick={mode === "static" ? toggleMode : undefined}
              aria-pressed={mode === "expression"}
              className={`px-2.5 py-1 text-body-sm font-medium transition-all
                focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none ${
                mode === "expression"
                  ? "bg-electric-indigo/20 text-electric-indigo"
                  : "text-orbflow-text-faint hover:text-orbflow-text-muted"
              }`}
            >
              Expression
            </button>
          </div>
        )}
      </div>

      {/* Wired state */}
      {isWired && wiredFrom && (
        <div className="flex items-center gap-2 rounded-lg border border-port-string/20 bg-port-string/[0.04] px-3.5 py-2.5">
          <NodeIcon name="link" className="w-3 h-3 text-port-string/50 shrink-0" />
          <div className="flex-1 min-w-0">
            <div className="text-body-sm font-mono text-port-string/80 truncate">
              {wiredFrom.sourceNodeLabel} &rarr; {wiredFrom.sourceField}
            </div>
            <div className="text-caption text-orbflow-text-ghost mt-0.5">Connected via edge</div>
          </div>
          <button
            onClick={() => setOverrideWired(true)}
            className="text-caption text-orbflow-text-ghost hover:text-orbflow-text-muted transition-colors shrink-0
              rounded focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none"
          >
            Override
          </button>
        </div>
      )}

      {/* Input fields */}
      {!isWired && (
        <>
          {mode === "static" ? (
            field.type === "credential" ? (
              <CredentialSelector
                value={mapping?.staticValue !== undefined ? String(mapping.staticValue) : ""}
                onChange={(id) => handleStaticChange(id)}
              />
            ) : field.enum ? (
              <select
                value={
                  mapping?.staticValue !== undefined
                    ? String(mapping.staticValue)
                    : field.default !== undefined
                      ? String(field.default)
                      : ""
                }
                onChange={(e) => handleStaticChange(e.target.value)}
                className={`w-full rounded-lg border border-orbflow-border bg-orbflow-surface px-3.5 py-2.5
                  text-body-lg focus:outline-none focus:border-electric-indigo/30
                  focus-visible:ring-2 focus-visible:ring-electric-indigo/50
                  hover:bg-orbflow-surface-hover cursor-pointer
                  transition-all duration-200 ${isUsingDefault ? "text-orbflow-text-faint italic" : "text-orbflow-text-secondary"}`}
              >
                {!field.required && (
                  <option value="" className="bg-orbflow-surface">Choose...</option>
                )}
                {field.enum.map((opt) => (
                  <option key={opt} value={opt} className="bg-orbflow-surface">{opt}</option>
                ))}
              </select>
            ) : field.type === "boolean" ? (
              <select
                value={
                  mapping?.staticValue !== undefined
                    ? String(mapping.staticValue)
                    : field.default !== undefined
                      ? String(field.default)
                      : ""
                }
                onChange={(e) => handleStaticChange(e.target.value)}
                className={`w-full rounded-lg border border-orbflow-border bg-orbflow-surface px-3.5 py-2.5
                  text-body-lg focus:outline-none focus:border-electric-indigo/30
                  focus-visible:ring-2 focus-visible:ring-electric-indigo/50
                  hover:bg-orbflow-surface-hover cursor-pointer
                  transition-all duration-200 ${isUsingDefault ? "text-orbflow-text-faint italic" : "text-orbflow-text-secondary"}`}
              >
                <option value="" className="bg-orbflow-surface">Choose...</option>
                <option value="true" className="bg-orbflow-surface">Yes (true)</option>
                <option value="false" className="bg-orbflow-surface">No (false)</option>
              </select>
            ) : (
              <input
                type={field.type === "number" ? "number" : "text"}
                value={
                  mapping?.mode === "static" && mapping.staticValue !== undefined
                    ? String(mapping.staticValue)
                    : field.default !== undefined
                      ? String(field.default)
                      : ""
                }
                onChange={(e) => handleStaticChange(e.target.value)}
                onFocus={() => onFocus?.(field.key)}
                placeholder={field.description || `Enter ${field.label.toLowerCase()}...`}
                className={`w-full rounded-lg border border-orbflow-border bg-orbflow-surface px-3.5 py-2.5
                  text-body-lg placeholder:text-orbflow-text-ghost
                  focus:outline-none focus:border-electric-indigo/30 focus:bg-orbflow-surface-hover
                  focus-visible:ring-2 focus-visible:ring-electric-indigo/50
                  transition-all duration-200 ${isUsingDefault ? "text-orbflow-text-faint italic" : "text-orbflow-text-secondary"}`}
              />
            )
          ) : (
            /* Expression mode */
            <div className="space-y-1.5" onFocus={() => onFocus?.(field.key)}>
              <CelExpressionEditor
                value={mapping?.celExpression || ""}
                upstream={upstream}
                onChange={(celExpression) =>
                  onChange({
                    targetKey: field.key,
                    mode: "expression",
                    celExpression,
                  })
                }
                onToggleBrowser={() => setShowBrowser(!showBrowser)}
                showBrowser={showBrowser}
              />

              {/* Field browser */}
              {showBrowser && (
                <div className="rounded-lg border border-orbflow-border bg-orbflow-bg p-2">
                  <FieldBrowser
                    upstream={upstream}
                    selectedPath={mapping?.celExpression}
                    onSelect={handleFieldSelect}
                  />
                </div>
              )}
            </div>
          )}
        </>
      )}

      {mode === "static" && !isWired && field.description && (
        <p className="text-body-sm text-orbflow-text-ghost mt-1.5 leading-relaxed">
          {field.description}
        </p>
      )}
    </div>
  );
}
