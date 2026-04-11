"use client";

import type { Dispatch } from "react";
import { NodeIcon } from "@/core/components/icons";
import { cn } from "@/lib/cn";
import type { CredentialTypeSchema } from "@/lib/api";
import { SchemaField } from "./schema-field";
import { TrustTierSelector } from "./trust-tier-selector";
import type { FormState, FormAction } from "./use-credential-form";

interface CredentialFormProps {
  form: FormState;
  dispatch: Dispatch<FormAction>;
  activeSchema: CredentialTypeSchema | null;
  credentialTypes: CredentialTypeSchema[];
  isNarrow: boolean;
  onTypeChange: (type: string) => void;
  onSave: () => void;
  onCancel: () => void;
}

export function CredentialForm({
  form,
  dispatch,
  activeSchema,
  credentialTypes,
  isNarrow,
  onTypeChange,
  onSave,
  onCancel,
}: CredentialFormProps) {
  const { name: formName, type: formType, description: formDescription, data: formData, accessTier, allowedDomains, saving, editingId } = form;
  return (
    <div className="flex-1 overflow-y-auto custom-scrollbar">
      <div className="max-w-lg mx-auto px-6 py-8 animate-fade-in-up">
        <div className="flex items-center gap-3 mb-6">
          {isNarrow && (
            <button
              onClick={onCancel}
              className="w-8 h-8 rounded-lg flex items-center justify-center text-orbflow-text-muted
                hover:bg-orbflow-surface-hover active:bg-orbflow-surface-hover/80 transition-colors
                focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none"
              aria-label="Back to list"
            >
              <NodeIcon name="arrow-left" className="w-4 h-4" />
            </button>
          )}
          <div>
            <h3 className="text-display font-semibold text-orbflow-text">
              {editingId ? "Edit Credential" : "New Credential"}
            </h3>
            <p className="text-body-sm text-orbflow-text-ghost mt-0.5">
              {editingId
                ? "Update connection settings and access policy"
                : "Add a new credential to use in your workflows"}
            </p>
          </div>
        </div>

        <div className="space-y-5">
          {/* Type selector -- card grid when creating, read-only when editing */}
          {!editingId ? (
            <div>
              <label className="text-body font-medium text-orbflow-text-faint block mb-2">
                Credential Type
              </label>
              <div className="grid grid-cols-2 gap-2">
                {credentialTypes.map((t) => (
                  <button
                    key={t.type}
                    onClick={() => onTypeChange(t.type)}
                    className={cn(
                      "flex items-center gap-2.5 px-3 py-3 rounded-xl border transition-all text-left",
                      formType === t.type
                        ? "border-electric-indigo/40 bg-electric-indigo/[0.06]"
                        : "border-orbflow-border hover:border-orbflow-border-hover hover:bg-orbflow-surface-hover"
                    )}
                  >
                    <div
                      className="w-8 h-8 rounded-lg flex items-center justify-center shrink-0"
                      style={{ backgroundColor: `${t.color}18` }}
                    >
                      <NodeIcon
                        name={t.icon}
                        className="w-4 h-4"
                        style={{ color: t.color }}
                      />
                    </div>
                    <div className="min-w-0">
                      <div className="text-body font-medium text-orbflow-text-secondary">
                        {t.name}
                      </div>
                      <div className="text-caption text-orbflow-text-ghost truncate">
                        {t.description}
                      </div>
                    </div>
                  </button>
                ))}
              </div>
            </div>
          ) : (
            <div className="flex items-center gap-2.5 px-3 py-2.5 rounded-xl bg-orbflow-add-btn-bg border border-orbflow-border">
              {activeSchema && (
                <div
                  className="w-7 h-7 rounded-lg flex items-center justify-center shrink-0"
                  style={{ backgroundColor: `${activeSchema.color}18` }}
                >
                  <NodeIcon
                    name={activeSchema.icon}
                    className="w-3.5 h-3.5"
                    style={{ color: activeSchema.color }}
                  />
                </div>
              )}
              <span className="text-body font-medium text-orbflow-text-muted">
                {activeSchema?.name || formType}
              </span>
            </div>
          )}

          {/* Name, Description, Fields -- only shown when type is selected */}
          {formType && (
            <>
              <div>
                <label className="text-body font-medium text-orbflow-text-faint block mb-1.5">
                  Name <span className="text-rose-400/70">*</span>
                </label>
                <input
                  type="text"
                  value={formName}
                  onChange={(e) => dispatch({ type: "SET_NAME", name: e.target.value })}
                  placeholder={
                    activeSchema
                      ? `e.g. My ${activeSchema.name}`
                      : "Credential name"
                  }
                  className="w-full rounded-lg border border-orbflow-border bg-orbflow-surface px-3.5 py-2.5
                    text-body-lg text-orbflow-text-secondary placeholder:text-orbflow-text-ghost
                    focus:outline-none focus:border-electric-indigo/30 focus-visible:ring-2 focus-visible:ring-electric-indigo/50 transition-colors"
                />
              </div>

              {/* Description */}
              <div>
                <label className="text-body font-medium text-orbflow-text-faint block mb-1.5">
                  Description
                </label>
                <input
                  type="text"
                  value={formDescription ?? ""}
                  onChange={(e) => dispatch({ type: "SET_DESCRIPTION", description: e.target.value })}
                  placeholder="Optional description"
                  className="w-full rounded-lg border border-orbflow-border bg-orbflow-surface px-3.5 py-2.5
                    text-body-lg text-orbflow-text-secondary placeholder:text-orbflow-text-ghost
                    focus:outline-none focus:border-electric-indigo/30 focus-visible:ring-2 focus-visible:ring-electric-indigo/50 transition-colors"
                />
              </div>

              {/* Schema-driven fields */}
              {activeSchema?.fields && activeSchema.fields.length > 0 ? (
                <div>
                  <div className="flex items-center gap-2 mb-3">
                    <NodeIcon
                      name="settings"
                      className="w-3.5 h-3.5 text-orbflow-text-faint"
                    />
                    <h4 className="text-body-sm font-bold uppercase tracking-[0.12em] text-orbflow-text-faint">
                      Connection Settings
                    </h4>
                    <div className="flex-1 h-px bg-orbflow-border" />
                  </div>
                  <div className="space-y-4">
                    {activeSchema.fields.map((field) => (
                      <SchemaField
                        key={field.key}
                        field={field}
                        value={formData[field.key]}
                        onChange={(val) => dispatch({ type: "SET_FIELD", key: field.key, value: val })}
                      />
                    ))}
                  </div>
                </div>
              ) : formType === "custom" ? (
                /* Custom type: free-form key-value pairs */
                <CustomFieldsSection
                  formData={formData}
                  dispatch={dispatch}
                />
              ) : null}

              {/* Trust Tier */}
              <TrustTierSelector
                value={accessTier}
                onChange={(tier) => dispatch({ type: "SET_ACCESS_TIER", tier })}
                allowedDomains={allowedDomains}
                onAllowedDomainsChange={(domains) => dispatch({ type: "SET_ALLOWED_DOMAINS", domains })}
              />

              {/* Actions */}
              <div className="flex items-center gap-3 pt-4 border-t border-orbflow-border">
                <button
                  onClick={onSave}
                  disabled={saving || !formName.trim() || !formType}
                  className={cn(
                    "flex items-center gap-2 px-5 py-2.5 rounded-lg text-body-lg font-medium transition-all",
                    saving || !formName.trim() || !formType
                      ? "bg-electric-indigo/20 text-electric-indigo/40 cursor-not-allowed"
                      : "bg-electric-indigo text-white hover:bg-electric-indigo/90 active:bg-electric-indigo/80 focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none"
                  )}
                >
                  {saving && (
                    <div className="w-3.5 h-3.5 border-2 border-white/30 border-t-white rounded-full animate-spin" />
                  )}
                  {saving ? "Saving..." : editingId ? "Update Credential" : "Create Credential"}
                </button>
                <button
                  onClick={onCancel}
                  disabled={saving}
                  className="px-5 py-2.5 rounded-lg text-body-lg font-medium text-orbflow-text-faint
                    hover:bg-orbflow-surface-hover active:bg-orbflow-surface-hover/80 transition-colors
                    focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none
                    disabled:opacity-50 disabled:cursor-not-allowed"
                >
                  Cancel
                </button>
              </div>
            </>
          )}
        </div>
      </div>
    </div>
  );
}

/** Custom type: free-form key-value pairs section */
function CustomFieldsSection({
  formData,
  dispatch,
}: {
  formData: Record<string, unknown>;
  dispatch: Dispatch<FormAction>;
}) {
  return (
    <div>
      <div className="flex items-center justify-between mb-2">
        <div className="flex items-center gap-2">
          <NodeIcon
            name="settings"
            className="w-3.5 h-3.5 text-orbflow-text-faint"
          />
          <h4 className="text-body-sm font-bold uppercase tracking-[0.12em] text-orbflow-text-faint">
            Custom Fields
          </h4>
        </div>
        <button
          onClick={() => dispatch({ type: "ADD_CUSTOM_FIELD", key: `field_${Object.keys(formData).length + 1}` })}
          className="text-body-sm text-electric-indigo hover:text-electric-indigo/80
            transition-colors font-medium"
        >
          + Add Field
        </button>
      </div>
      <div className="space-y-2">
        {Object.entries(formData).map(([key, value]) => (
          <div key={key} className="flex gap-2">
            <input
              type="text"
              value={key}
              onChange={(e) => {
                const newKey = e.target.value;
                const next: Record<string, unknown> = {};
                for (const [k, v] of Object.entries(formData)) {
                  next[k === key ? newKey : k] = v;
                }
                dispatch({ type: "SET_DATA", data: next });
              }}
              placeholder="Key"
              className="flex-1 rounded-lg border border-orbflow-border bg-orbflow-surface px-3 py-2
                text-body font-mono text-orbflow-text-secondary placeholder:text-orbflow-text-ghost
                focus:outline-none focus:border-electric-indigo/30 focus-visible:ring-2 focus-visible:ring-electric-indigo/50 transition-colors"
            />
            <input
              type="password"
              value={String(value ?? "")}
              onChange={(e) => dispatch({ type: "SET_FIELD", key, value: e.target.value })}
              placeholder="Value"
              className="flex-1 rounded-lg border border-orbflow-border bg-orbflow-surface px-3 py-2
                text-body font-mono text-orbflow-text-secondary placeholder:text-orbflow-text-ghost
                focus:outline-none focus:border-electric-indigo/30 focus-visible:ring-2 focus-visible:ring-electric-indigo/50 transition-colors"
            />
            <button
              onClick={() => dispatch({ type: "REMOVE_CUSTOM_FIELD", key })}
              className="p-2 rounded-lg text-orbflow-text-ghost hover:text-rose-400
                hover:bg-rose-400/10 transition-colors"
            >
              <NodeIcon name="x" className="w-3 h-3" />
            </button>
          </div>
        ))}
        {Object.keys(formData).length === 0 && (
          <p className="text-body-sm text-orbflow-text-ghost py-3 text-center border border-dashed border-orbflow-border rounded-lg">
            No fields yet. Click &quot;+ Add Field&quot; to add key-value pairs.
          </p>
        )}
      </div>
    </div>
  );
}
