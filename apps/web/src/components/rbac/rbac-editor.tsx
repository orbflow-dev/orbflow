"use client";

import { useState, useEffect, useCallback, useMemo, useRef } from "react";
import { NodeIcon } from "@/core/components/icons";
import { cn } from "@/lib/cn";
import { api } from "@/lib/api";
import type { RbacPolicy, Role, PolicyBinding, PolicyScope, Permission } from "@orbflow/core";
import { RoleForm } from "./role-form";
import { SubjectComboBox } from "./subject-combo-box";

/* =======================================================
   Permission badge color mapping
   ======================================================= */

const PERMISSION_COLORS: Record<Permission, { bg: string; text: string; icon: string }> = {
  view:               { bg: "bg-sky-500/10",     text: "text-sky-400",     icon: "search" },
  edit:               { bg: "bg-amber-500/10",   text: "text-amber-400",   icon: "edit" },
  execute:            { bg: "bg-emerald-500/10", text: "text-emerald-400", icon: "play" },
  approve:            { bg: "bg-violet-500/10",  text: "text-violet-400",  icon: "check" },
  delete:             { bg: "bg-rose-500/10",    text: "text-rose-400",    icon: "trash" },
  manage_credentials: { bg: "bg-orange-500/10",  text: "text-orange-400",  icon: "key" },
  admin:              { bg: "bg-fuchsia-500/10", text: "text-fuchsia-400", icon: "shield" },
};

const ALL_PERMISSIONS: Permission[] = [
  "view", "edit", "execute", "approve", "delete", "manage_credentials", "admin",
];

/* Role tier colors for visual hierarchy */
const ROLE_TIER_COLORS: Record<string, {
  border: string; accent: string; bar: string; iconBg: string; icon: string;
}> = {
  admin:  { border: "border-fuchsia-500/20", accent: "text-fuchsia-400", bar: "bg-fuchsia-500/60", iconBg: "bg-fuchsia-500/10", icon: "shield" },
  editor: { border: "border-amber-500/20",   accent: "text-amber-400",   bar: "bg-amber-500/60",   iconBg: "bg-amber-500/10",   icon: "edit" },
  viewer: { border: "border-sky-500/20",     accent: "text-sky-400",     bar: "bg-sky-500/60",     iconBg: "bg-sky-500/10",     icon: "search" },
};

function getRoleTier(role: Role): string {
  if (role.permissions.includes("admin")) return "admin";
  if (role.permissions.includes("edit")) return "editor";
  return "viewer";
}

/* =======================================================
   Sub-components
   ======================================================= */

function PermissionBadge({ permission }: { permission: Permission }) {
  const colors = PERMISSION_COLORS[permission] ?? { bg: "bg-orbflow-surface-hover", text: "text-orbflow-text-faint", icon: "default" };
  return (
    <span className={cn(
      "inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-micro font-semibold tracking-wide",
      colors.bg, colors.text
    )}>
      <NodeIcon name={colors.icon} className="w-2.5 h-2.5" />
      {permission.replaceAll("_", " ")}
    </span>
  );
}

function ScopeTag({ scope }: { scope: PolicyScope }) {
  const config = {
    global: { icon: "globe", label: "Global", color: "text-emerald-400 bg-emerald-500/10" },
    workflow: { icon: "workflow", label: `Workflow`, color: "text-amber-400 bg-amber-500/10" },
    node: { icon: "zap", label: `Node`, color: "text-sky-400 bg-sky-500/10" },
  };
  const c = config[scope.type];

  return (
    <div className={cn("inline-flex items-center gap-1.5 rounded-lg px-2 py-1 text-xs font-medium", c.color)}>
      <NodeIcon name={c.icon} className="w-3 h-3" />
      <span>{c.label}</span>
      {scope.type === "workflow" && (
        <span className="font-mono text-orbflow-text-ghost truncate max-w-[120px]">{scope.workflow_id}</span>
      )}
      {scope.type === "node" && (
        <span className="font-mono text-orbflow-text-ghost truncate max-w-[120px]">
          {scope.workflow_id}/{scope.node_id}
        </span>
      )}
    </div>
  );
}

/* --- Role Cards ---------------------------------------- */

function RoleCard({
  role,
  bindingCount,
  onEdit,
  onDelete,
}: {
  role: Role;
  bindingCount: number;
  onEdit?: (role: Role) => void;
  onDelete?: () => void;
}) {
  const [isEditing, setIsEditing] = useState(false);
  const [editName, setEditName] = useState(role.name);
  const [editDescription, setEditDescription] = useState(role.description ?? "");
  const [editPermissions, setEditPermissions] = useState<Permission[]>([...role.permissions]);
  const tier = getRoleTier(role);
  const tierColor = ROLE_TIER_COLORS[tier] ?? ROLE_TIER_COLORS.viewer;

  const editInputClasses =
    "w-full rounded-lg bg-orbflow-surface border border-orbflow-border text-orbflow-text-secondary text-body-sm px-3 py-2 " +
    "placeholder:text-orbflow-text-ghost/50 focus:outline-none focus:ring-2 focus:ring-electric-indigo/50 focus:border-electric-indigo/40 transition-colors";

  function handleSaveInline() {
    if (!onEdit || editName.trim().length < 3 || editPermissions.length === 0) return;
    onEdit({
      ...role,
      name: editName.trim(),
      description: editDescription.trim(),
      permissions: editPermissions,
    });
    setIsEditing(false);
  }

  function handleCancelInline() {
    setEditName(role.name);
    setEditDescription(role.description ?? "");
    setEditPermissions([...role.permissions]);
    setIsEditing(false);
  }

  function toggleEditPermission(perm: Permission) {
    setEditPermissions((prev) =>
      prev.includes(perm) ? prev.filter((p) => p !== perm) : [...prev, perm]
    );
  }

  /* ---- Inline edit mode ---- */
  if (isEditing) {
    return (
      <div className="rounded-xl border border-electric-indigo/25 bg-electric-indigo/[0.03] overflow-hidden animate-scale-in">
        <div className="p-4 space-y-3">
          <div className="flex items-center justify-between">
            <h4 className="text-body font-semibold text-orbflow-text-secondary">Edit Role</h4>
            <button
              type="button"
              onClick={handleCancelInline}
              className="p-1.5 rounded-lg text-orbflow-text-ghost hover:text-orbflow-text-secondary hover:bg-orbflow-surface-hover transition-colors"
              aria-label="Cancel editing"
            >
              <NodeIcon name="x" className="w-3.5 h-3.5" />
            </button>
          </div>
          <div className="space-y-1.5">
            <label htmlFor={`edit-name-${role.id}`} className="text-caption uppercase tracking-wider font-semibold text-orbflow-text-ghost">
              Name
            </label>
            <input
              id={`edit-name-${role.id}`}
              type="text"
              value={editName}
              onChange={(e) => setEditName(e.target.value)}
              className={editInputClasses}
              autoFocus
              onKeyDown={(e) => {
                if (e.key === "Enter") handleSaveInline();
                if (e.key === "Escape") handleCancelInline();
              }}
            />
          </div>
          <div className="space-y-1.5">
            <label htmlFor={`edit-desc-${role.id}`} className="text-caption uppercase tracking-wider font-semibold text-orbflow-text-ghost">
              Description
            </label>
            <input
              id={`edit-desc-${role.id}`}
              type="text"
              value={editDescription}
              onChange={(e) => setEditDescription(e.target.value)}
              className={editInputClasses}
              onKeyDown={(e) => {
                if (e.key === "Enter") handleSaveInline();
                if (e.key === "Escape") handleCancelInline();
              }}
            />
          </div>
          <div className="space-y-1.5">
            <span className="text-caption uppercase tracking-wider font-semibold text-orbflow-text-ghost">
              Permissions
            </span>
            <div className="flex flex-wrap gap-1.5">
              {ALL_PERMISSIONS.map((perm) => {
                const selected = editPermissions.includes(perm);
                const colors = PERMISSION_COLORS[perm];
                return (
                  <button
                    key={perm}
                    type="button"
                    onClick={() => toggleEditPermission(perm)}
                    aria-pressed={selected}
                    className={cn(
                      "inline-flex items-center gap-1 px-2.5 py-1 rounded-lg text-xs font-medium transition-colors duration-200 cursor-pointer",
                      selected
                        ? cn(colors.bg, colors.text)
                        : "bg-orbflow-surface-hover/50 text-orbflow-text-ghost/40"
                    )}
                  >
                    <NodeIcon name={colors.icon} className="w-3 h-3" />
                    {perm.replaceAll("_", " ")}
                    {selected && <NodeIcon name="check" className="w-2.5 h-2.5 ml-0.5" />}
                  </button>
                );
              })}
            </div>
          </div>
          <div className="flex items-center gap-2 pt-3 border-t border-electric-indigo/15">
            <button
              type="button"
              onClick={handleSaveInline}
              disabled={editName.trim().length < 3 || editPermissions.length === 0}
              className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-body-sm font-medium transition-colors
                bg-emerald-600 text-white hover:bg-emerald-500
                disabled:opacity-40 disabled:cursor-not-allowed disabled:hover:bg-emerald-600
                focus-visible:ring-2 focus-visible:ring-emerald-400/50 focus-visible:outline-none"
            >
              <NodeIcon name="check" className="w-3 h-3" />Save
            </button>
            <button
              type="button"
              onClick={handleCancelInline}
              className="px-3 py-1.5 rounded-lg text-body-sm font-medium text-orbflow-text-ghost hover:text-orbflow-text-secondary transition-colors"
            >
              Cancel
            </button>
          </div>
        </div>
      </div>
    );
  }

  /* ---- View mode ---- */
  return (
    <div className={cn(
      "group relative rounded-xl border overflow-hidden transition-all duration-200",
      "hover:border-orbflow-border-hover",
      tierColor.border,
    )}>
      {/* Tier accent bar */}
      <div className={cn("absolute left-0 top-0 bottom-0 w-[3px] rounded-l-xl", tierColor.bar)} />

      <div className="p-4 pl-5">
        {/* Header */}
        <div className="flex items-start gap-3">
          <div className={cn(
            "flex items-center justify-center w-9 h-9 rounded-lg shrink-0",
            tierColor.iconBg,
          )}>
            <NodeIcon name={tierColor.icon} className={cn("w-4 h-4", tierColor.accent)} />
          </div>
          <div className="flex-1 min-w-0">
            <div className="flex items-center gap-2">
              <span className="text-body font-semibold text-orbflow-text-secondary truncate">{role.name}</span>
              {role.builtin ? (
                <span className="px-1.5 py-0.5 rounded text-micro font-mono bg-orbflow-surface-hover/60 text-orbflow-text-ghost shrink-0">
                  System
                </span>
              ) : (
                <span className="px-1.5 py-0.5 rounded text-micro font-medium bg-electric-indigo/8 text-electric-indigo shrink-0">
                  Custom
                </span>
              )}
            </div>
            {role.description && (
              <p className="text-body-sm text-orbflow-text-faint mt-0.5 line-clamp-2">{role.description}</p>
            )}
          </div>
          {/* Actions — always visible for custom roles */}
          {(onEdit || onDelete) && (
            <div className="flex items-center gap-0.5 shrink-0">
              {onEdit && (
                <button
                  type="button"
                  onClick={() => setIsEditing(true)}
                  aria-label={`Edit ${role.name}`}
                  className="p-1.5 rounded-lg text-orbflow-text-ghost/60 hover:text-electric-indigo hover:bg-electric-indigo/10 transition-colors
                    focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none"
                >
                  <NodeIcon name="edit" className="w-3.5 h-3.5" />
                </button>
              )}
              {onDelete && (
                <button
                  type="button"
                  onClick={onDelete}
                  aria-label={`Delete ${role.name}`}
                  className="p-1.5 rounded-lg text-orbflow-text-ghost/60 hover:text-rose-400 hover:bg-rose-500/10 transition-colors
                    focus-visible:ring-2 focus-visible:ring-rose-400/50 focus-visible:outline-none"
                >
                  <NodeIcon name="trash" className="w-3.5 h-3.5" />
                </button>
              )}
            </div>
          )}
        </div>

        {/* Permissions — always visible */}
        <div className="flex flex-wrap gap-1.5 mt-3">
          {role.permissions.map((perm) => {
            const colors = PERMISSION_COLORS[perm];
            return (
              <span
                key={perm}
                className={cn(
                  "inline-flex items-center gap-1 px-2 py-0.5 rounded-md text-micro font-medium",
                  colors.bg, colors.text,
                )}
              >
                <NodeIcon name={colors.icon} className="w-2.5 h-2.5" />
                {perm.replaceAll("_", " ")}
              </span>
            );
          })}
        </div>

        {/* Footer */}
        <div className="flex items-center gap-4 mt-3 pt-2.5 border-t border-orbflow-border/15">
          <div className="flex items-center gap-1.5">
            <NodeIcon name="users" className="w-3 h-3 text-orbflow-text-ghost" />
            <span className="text-body-sm text-orbflow-text-ghost tabular-nums">
              {bindingCount} {bindingCount === 1 ? "member" : "members"}
            </span>
          </div>
          <span className="text-body-sm text-orbflow-text-ghost/60 font-mono ml-auto truncate max-w-[100px]" title={role.id}>
            {role.id}
          </span>
        </div>
      </div>
    </div>
  );
}

/* --- Binding Card (mobile) / Row (desktop) ------------ */

function BindingTableRow({
  binding,
  roleName,
  onRemove,
}: {
  binding: PolicyBinding;
  roleName: string;
  onRemove: () => void;
}) {
  return (
    <tr className="border-b border-orbflow-border/20 hover:bg-orbflow-surface-hover/30 transition-colors group">
      <td className="px-4 py-3">
        <div className="flex items-center gap-2">
          <div className="w-6 h-6 rounded-full bg-electric-indigo/10 flex items-center justify-center shrink-0">
            <span className="text-micro font-bold text-electric-indigo uppercase">
              {binding.subject.charAt(0)}
            </span>
          </div>
          <span className="text-body font-medium text-orbflow-text-secondary truncate">{binding.subject}</span>
        </div>
      </td>
      <td className="px-4 py-3">
        <span className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-lg bg-electric-indigo/8 border border-electric-indigo/15 text-body-sm font-medium text-electric-indigo">
          <NodeIcon name="shield" className="w-3 h-3" />
          {roleName}
        </span>
      </td>
      <td className="px-4 py-3">
        <ScopeTag scope={binding.scope} />
      </td>
      <td className="px-4 py-3">
        <button
          type="button"
          onClick={onRemove}
          aria-label={`Remove binding for ${binding.subject}`}
          className="opacity-0 group-hover:opacity-100 flex items-center justify-center w-7 h-7 rounded-lg transition-all
            text-rose-400/70 hover:text-rose-400 hover:bg-rose-500/10
            focus-visible:opacity-100 focus-visible:ring-2 focus-visible:ring-rose-400/50 focus-visible:outline-none"
        >
          <NodeIcon name="x" className="w-3.5 h-3.5" />
        </button>
      </td>
    </tr>
  );
}

function BindingMobileCard({
  binding,
  roleName,
  onRemove,
}: {
  binding: PolicyBinding;
  roleName: string;
  onRemove: () => void;
}) {
  return (
    <div className="border-b border-orbflow-border/20 px-4 py-3 flex items-start gap-3">
      <div className="w-8 h-8 rounded-full bg-electric-indigo/10 flex items-center justify-center shrink-0 mt-0.5">
        <span className="text-xs font-bold text-electric-indigo uppercase">
          {binding.subject.charAt(0)}
        </span>
      </div>
      <div className="flex-1 min-w-0 space-y-1.5">
        <span className="text-body font-medium text-orbflow-text-secondary block truncate">{binding.subject}</span>
        <div className="flex flex-wrap items-center gap-1.5">
          <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-md bg-electric-indigo/8 border border-electric-indigo/15 text-micro font-medium text-electric-indigo">
            <NodeIcon name="shield" className="w-2.5 h-2.5" />
            {roleName}
          </span>
          <ScopeTag scope={binding.scope} />
        </div>
      </div>
      <button
        type="button"
        onClick={onRemove}
        aria-label={`Remove binding for ${binding.subject}`}
        className="flex items-center justify-center w-8 h-8 rounded-lg shrink-0
          text-rose-400/70 hover:text-rose-400 hover:bg-rose-500/10 active:bg-rose-500/15
          focus-visible:ring-2 focus-visible:ring-rose-400/50 focus-visible:outline-none"
      >
        <NodeIcon name="x" className="w-3.5 h-3.5" />
      </button>
    </div>
  );
}

/* --- Add Binding Form ---------------------------------- */

interface AddBindingFormProps {
  readonly roles: Role[];
  readonly knownSubjects: string[];
  readonly onAdd: (binding: PolicyBinding) => void;
  readonly onCancel: () => void;
}

function AddBindingForm({ roles, knownSubjects, onAdd, onCancel }: AddBindingFormProps) {
  const [subject, setSubject] = useState("");
  const [roleId, setRoleId] = useState(roles[0]?.id ?? "");
  const [scopeType, setScopeType] = useState<"global" | "workflow" | "node">("global");
  const [workflowId, setWorkflowId] = useState("");
  const [nodeId, setNodeId] = useState("");

  const inputClasses =
    "w-full rounded-lg bg-orbflow-surface border border-orbflow-border text-orbflow-text-secondary text-body-sm px-3 py-2 " +
    "placeholder:text-orbflow-text-ghost/50 focus:outline-none focus:ring-2 focus:ring-electric-indigo/50 focus:border-electric-indigo/40 transition-colors";

  const canSubmit =
    subject.trim() !== "" &&
    roleId !== "" &&
    (scopeType === "global" || workflowId.trim() !== "") &&
    (scopeType !== "node" || nodeId.trim() !== "");

  function handleSubmit() {
    if (!canSubmit) return;

    let scope: PolicyScope;
    switch (scopeType) {
      case "workflow":
        scope = { type: "workflow", workflow_id: workflowId.trim() };
        break;
      case "node":
        scope = { type: "node", workflow_id: workflowId.trim(), node_id: nodeId.trim() };
        break;
      default:
        scope = { type: "global" };
    }

    onAdd({ subject: subject.trim(), role_id: roleId, scope });
  }

  const selectedRole = roles.find((r) => r.id === roleId);

  return (
    <div className="rounded-xl border border-electric-indigo/20 bg-electric-indigo/[0.02] p-4 space-y-4 animate-scale-in">
      <h4 className="text-body font-semibold text-orbflow-text-secondary">Add Binding</h4>

      <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
        {/* Subject */}
        <div className="space-y-1.5">
          <label htmlFor="binding-subject" className="text-caption uppercase tracking-wider font-semibold text-orbflow-text-ghost">
            Subject (User / Team)
          </label>
          <SubjectComboBox
            id="binding-subject"
            value={subject}
            onChange={setSubject}
            suggestions={knownSubjects}
            placeholder="e.g. user@example.com or team-name"
            className={inputClasses}
            autoFocus
            onKeyDown={(e) => {
              if (e.key === "Enter") handleSubmit();
              if (e.key === "Escape") onCancel();
            }}
          />
        </div>

        {/* Role */}
        <div className="space-y-1.5">
          <label htmlFor="binding-role" className="text-caption uppercase tracking-wider font-semibold text-orbflow-text-ghost">
            Role
          </label>
          <select
            id="binding-role"
            value={roleId}
            onChange={(e) => setRoleId(e.target.value)}
            className={cn(inputClasses, "appearance-none cursor-pointer")}
          >
            {roles.length === 0 && (
              <option value="" disabled>No roles yet -- define one above</option>
            )}
            {roles.length > 0 && (
              <option value="" disabled>Select a role...</option>
            )}
            {roles.map((r) => (
              <option key={r.id} value={r.id}>
                {r.name}
              </option>
            ))}
          </select>
          {/* Role preview */}
          {selectedRole && (
            <div className="flex flex-wrap gap-1 mt-1">
              {selectedRole.permissions.slice(0, 4).map((p) => (
                <PermissionBadge key={p} permission={p} />
              ))}
              {selectedRole.permissions.length > 4 && (
                <span className="text-micro text-orbflow-text-ghost">
                  +{selectedRole.permissions.length - 4} more
                </span>
              )}
            </div>
          )}
        </div>
      </div>

      {/* Scope */}
      <fieldset className="space-y-1.5">
        <legend className="text-caption uppercase tracking-wider font-semibold text-orbflow-text-ghost">
          Scope
        </legend>
        <div className="flex flex-wrap gap-2">
          {(["global", "workflow", "node"] as const).map((st) => {
            const scopeConfig = {
              global: { icon: "globe", label: "Global" },
              workflow: { icon: "workflow", label: "Workflow" },
              node: { icon: "zap", label: "Node" },
            };
            const sc = scopeConfig[st];
            return (
              <button
                key={st}
                type="button"
                onClick={() => setScopeType(st)}
                aria-pressed={scopeType === st}
                className={cn(
                  "flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-body-sm font-medium transition-colors",
                  scopeType === st
                    ? "bg-electric-indigo/15 text-electric-indigo border border-electric-indigo/30"
                    : "bg-orbflow-surface border border-orbflow-border text-orbflow-text-ghost hover:text-orbflow-text-faint hover:border-orbflow-border-hover"
                )}
              >
                <NodeIcon name={sc.icon} className="w-3 h-3" />
                {sc.label}
              </button>
            );
          })}
        </div>

        {/* Conditional scope fields */}
        {(scopeType === "workflow" || scopeType === "node") && (
          <div className="grid grid-cols-1 gap-2 mt-2 sm:grid-cols-2">
            <div>
              <label htmlFor="binding-workflow-id" className="sr-only">Workflow ID</label>
              <input
                id="binding-workflow-id"
                type="text"
                value={workflowId}
                onChange={(e) => setWorkflowId(e.target.value)}
                placeholder="Workflow ID"
                className={inputClasses}
              />
            </div>
            {scopeType === "node" && (
              <div>
                <label htmlFor="binding-node-id" className="sr-only">Node ID</label>
                <input
                  id="binding-node-id"
                  type="text"
                  value={nodeId}
                  onChange={(e) => setNodeId(e.target.value)}
                  placeholder="Node ID"
                  className={inputClasses}
                />
              </div>
            )}
          </div>
        )}
      </fieldset>

      {/* Actions */}
      <div className="flex items-center gap-2 pt-2 border-t border-orbflow-border/30">
        <button
          type="button"
          onClick={handleSubmit}
          disabled={!canSubmit}
          className="flex items-center gap-1.5 px-4 py-2 rounded-lg text-body-sm font-medium transition-colors
            bg-emerald-600 text-white hover:bg-emerald-500
            disabled:opacity-40 disabled:cursor-not-allowed disabled:hover:bg-emerald-600
            focus-visible:ring-2 focus-visible:ring-emerald-400/50 focus-visible:outline-none"
        >
          <NodeIcon name="plus" className="w-3 h-3" />Add Binding
        </button>
        <button
          type="button"
          onClick={onCancel}
          className="px-4 py-2 rounded-lg text-body-sm font-medium text-orbflow-text-ghost hover:text-orbflow-text-secondary transition-colors"
        >
          Cancel
        </button>
      </div>
    </div>
  );
}

/* =======================================================
   Validation helpers
   ======================================================= */

function hasAdminBinding(bindings: PolicyBinding[], roles: Role[]): boolean {
  const adminRoleIds = new Set(
    roles.filter((r) => r.permissions.includes("admin")).map((r) => r.id)
  );
  return bindings.some((b) => adminRoleIds.has(b.role_id));
}

/* =======================================================
   Main RBAC Editor
   ======================================================= */

export function RbacEditor() {
  const [policy, setPolicy] = useState<RbacPolicy | null>(null);
  const [editedBindings, setEditedBindings] = useState<PolicyBinding[]>([]);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [saveSuccess, setSaveSuccess] = useState(false);
  const [showAddForm, setShowAddForm] = useState(false);
  const [searchQuery, setSearchQuery] = useState("");
  const [pendingRemoveIdx, setPendingRemoveIdx] = useState<number | null>(null);
  const successTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const [editedRoles, setEditedRoles] = useState<Role[]>([]);
  const [showRoleForm, setShowRoleForm] = useState(false);
  const [editingRoleId, setEditingRoleId] = useState<string | null>(null);
  const [pendingDeleteRoleId, setPendingDeleteRoleId] = useState<string | null>(null);

  useEffect(() => {
    return () => {
      if (successTimerRef.current) clearTimeout(successTimerRef.current);
    };
  }, []);

  const isDirty = useMemo(() => {
    if (!policy) return false;
    return JSON.stringify(policy.bindings) !== JSON.stringify(editedBindings) ||
           JSON.stringify(policy.roles) !== JSON.stringify(editedRoles);
  }, [policy, editedBindings, editedRoles]);

  const validationError = useMemo(() => {
    if (!policy) return null;
    if (!hasAdminBinding(editedBindings, editedRoles)) {
      return "At least one binding must use a role with admin permission";
    }
    const emptyPermsRole = editedRoles.find(r => !r.builtin && r.permissions.length === 0);
    if (emptyPermsRole) {
      return `Role "${emptyPermsRole.name}" must have at least one permission`;
    }
    return null;
  }, [policy, editedBindings, editedRoles]);

  const fetchPolicy = useCallback(async () => {
    setLoading(true);
    setError(null);
    setSaveError(null);
    setSaveSuccess(false);
    try {
      const result = await api.rbac.getPolicy();
      // Deduplicate roles by id to prevent React key collisions
      const seenRoleIds = new Set<string>();
      const dedupedRoles = result.roles.filter((r) => {
        if (seenRoleIds.has(r.id)) return false;
        seenRoleIds.add(r.id);
        return true;
      });
      setPolicy({ ...result, roles: dedupedRoles });
      setEditedBindings(result.bindings);
      setEditedRoles(dedupedRoles);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load policy");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchPolicy();
  }, [fetchPolicy]);

  const handleAddBinding = useCallback((binding: PolicyBinding) => {
    setEditedBindings((prev) => [...prev, binding]);
    setShowAddForm(false);
    setSaveError(null);
    setSaveSuccess(false);
  }, []);

  const handleRemoveBinding = useCallback((index: number) => {
    setEditedBindings((prev) => prev.filter((_, i) => i !== index));
    setPendingRemoveIdx(null);
    setSaveError(null);
    setSaveSuccess(false);
  }, []);

  const handleAddRole = useCallback((role: Role) => {
    setEditedRoles(prev => [...prev, role]);
    setShowRoleForm(false);
  }, []);

  const handleUpdateRole = useCallback((updatedRole: Role) => {
    setEditedRoles(prev => prev.map(r => r.id === updatedRole.id ? updatedRole : r));
    setEditingRoleId(null);
  }, []);

  const handleDeleteRole = useCallback((roleId: string) => {
    setEditedRoles(prev => prev.filter(r => r.id !== roleId));
    setEditedBindings(prev => prev.filter(b => b.role_id !== roleId));
    setPendingDeleteRoleId(null);
  }, []);

  const handleDiscard = useCallback(() => {
    if (!policy) return;
    setEditedBindings(policy.bindings);
    setEditedRoles(policy.roles);
    setShowAddForm(false);
    setShowRoleForm(false);
    setEditingRoleId(null);
    setPendingDeleteRoleId(null);
    setSaveError(null);
    setSaveSuccess(false);
  }, [policy]);

  const handleSave = useCallback(async () => {
    if (!policy || !isDirty || validationError) return;
    setSaving(true);
    setSaveError(null);
    setSaveSuccess(false);
    try {
      const updatedPolicy: RbacPolicy = { roles: editedRoles, bindings: editedBindings };
      const result = await api.rbac.updatePolicy(updatedPolicy);
      setPolicy(result);
      setEditedBindings(result.bindings);
      setEditedRoles(result.roles);
      if (successTimerRef.current) clearTimeout(successTimerRef.current);
      setSaveSuccess(true);
      successTimerRef.current = setTimeout(() => setSaveSuccess(false), 3000);
    } catch (err) {
      const message = err instanceof Error ? err.message : "Failed to save policy";
      setSaveError(message);
    } finally {
      setSaving(false);
    }
  }, [policy, editedRoles, editedBindings, isDirty, validationError]);

  // Binding count per role for display in role cards
  const bindingCountByRole = useMemo(() => {
    const counts: Record<string, number> = {};
    for (const b of editedBindings) {
      counts[b.role_id] = (counts[b.role_id] ?? 0) + 1;
    }
    return counts;
  }, [editedBindings]);

  // Filtered bindings based on search
  const filteredBindings = useMemo(() => {
    if (!searchQuery.trim()) return editedBindings;
    const q = searchQuery.toLowerCase();
    return editedBindings.filter((b) =>
      b.subject.toLowerCase().includes(q) ||
      b.role_id.toLowerCase().includes(q)
    );
  }, [editedBindings, searchQuery]);

  const roleNameMap = useMemo(() => {
    return new Map(editedRoles.map((r) => [r.id, r.name]));
  }, [editedRoles]);

  const knownSubjects = useMemo(() => {
    return [...new Set(editedBindings.map((b) => b.subject))].sort();
  }, [editedBindings]);

  if (loading) {
    return (
      <div className="flex items-center justify-center py-20">
        <div className="flex items-center gap-2.5 text-orbflow-text-faint">
          <div className="w-4 h-4 animate-spin rounded-full border-2 border-orbflow-border border-t-electric-indigo" />
          <span className="text-body">Loading access policy...</span>
        </div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="flex flex-col items-center justify-center py-20 gap-3">
        <div className="w-12 h-12 rounded-xl bg-red-500/10 flex items-center justify-center">
          <NodeIcon name="alert-triangle" className="w-6 h-6 text-rose-400" />
        </div>
        <p className="text-body text-rose-400/80">{error}</p>
        <button
          onClick={fetchPolicy}
          className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-body-sm font-medium
            bg-orbflow-surface-hover/50 border border-orbflow-border text-orbflow-text-faint
            hover:text-orbflow-text-secondary hover:border-orbflow-border-hover transition-colors"
        >
          <NodeIcon name="repeat" className="w-3 h-3" />Retry
        </button>
      </div>
    );
  }

  if (!policy) return null;

  return (
    <div className="flex flex-col gap-6 p-4 sm:p-6 max-w-5xl mx-auto">
      {/* -- Header ---------------------------- */}
      <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        <div className="flex items-center gap-3">
          <div className="w-10 h-10 rounded-xl bg-electric-indigo/10 flex items-center justify-center shrink-0">
            <NodeIcon name="shield" className="w-5 h-5 text-electric-indigo" />
          </div>
          <div>
            <h2 className="text-display font-bold text-orbflow-text-secondary tracking-tight">Access Control</h2>
            <p className="text-body-sm text-orbflow-text-ghost mt-0.5">
              Manage roles, permissions, and who can access what
            </p>
          </div>
        </div>
        <div className="flex items-center gap-2">
          {isDirty && (
            <span
              role="status"
              className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-body-sm font-medium bg-amber-500/10 border border-amber-500/20 text-amber-400 animate-scale-in"
            >
              <span className="w-1.5 h-1.5 rounded-full bg-amber-400 animate-pulse-soft" />
              Unsaved changes
            </span>
          )}
          {saveSuccess && (
            <span
              role="status"
              aria-live="polite"
              className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-body-sm font-medium bg-emerald-500/10 border border-emerald-500/20 text-emerald-400 animate-scale-in"
            >
              <NodeIcon name="check" className="w-3 h-3" />
              Saved
            </span>
          )}
          <button
            onClick={fetchPolicy}
            aria-label="Refresh policy"
            className="flex items-center gap-1.5 rounded-lg px-3 py-2 text-body font-medium transition-colors
              bg-orbflow-surface-hover/50 border border-orbflow-border text-orbflow-text-faint
              hover:text-orbflow-text-secondary hover:border-orbflow-border-hover"
          >
            <NodeIcon name="repeat" className="w-3 h-3" />Refresh
          </button>
        </div>
      </div>

      {/* -- Error / Validation banners ------- */}
      {saveError && (
        <div
          role="alert"
          className="flex items-center gap-2 px-4 py-3 rounded-xl bg-rose-500/10 border border-rose-500/20 text-rose-400 animate-scale-in"
        >
          <NodeIcon name="alert-triangle" className="w-4 h-4 shrink-0" />
          <p className="text-body-sm flex-1">{saveError}</p>
          <button
            type="button"
            onClick={() => setSaveError(null)}
            className="ml-auto text-rose-400/60 hover:text-rose-400 transition-colors p-1 rounded-md
              focus-visible:ring-2 focus-visible:ring-rose-400/50 focus-visible:outline-none"
            aria-label="Dismiss error"
          >
            <NodeIcon name="x" className="w-3.5 h-3.5" />
          </button>
        </div>
      )}

      {isDirty && validationError && (
        <div
          role="alert"
          className="flex items-center gap-2 px-4 py-3 rounded-xl bg-amber-500/10 border border-amber-500/20 text-amber-400"
        >
          <NodeIcon name="alert-triangle" className="w-4 h-4 shrink-0" />
          <p className="text-body-sm">{validationError}</p>
        </div>
      )}

      {/* -- Summary stats ---------------------- */}
      <div className="grid grid-cols-1 gap-3 sm:grid-cols-3">
        <div className="rounded-xl border border-orbflow-border bg-orbflow-surface p-4">
          <div className="flex items-center gap-1.5 mb-2">
            <NodeIcon name="shield" className="w-3 h-3 text-fuchsia-400" />
            <p className="text-xs font-medium uppercase tracking-wider text-orbflow-text-ghost">Roles</p>
          </div>
          <p className="text-2xl font-bold text-orbflow-text-secondary tabular-nums">{editedRoles.length}</p>
        </div>
        <div className="rounded-xl border border-orbflow-border bg-orbflow-surface p-4">
          <div className="flex items-center gap-1.5 mb-2">
            <NodeIcon name="users" className="w-3 h-3 text-electric-indigo" />
            <p className="text-xs font-medium uppercase tracking-wider text-orbflow-text-ghost">Bindings</p>
          </div>
          <p className="text-2xl font-bold text-orbflow-text-secondary tabular-nums">{editedBindings.length}</p>
        </div>
        <div className="rounded-xl border border-orbflow-border bg-orbflow-surface p-4">
          <div className="flex items-center gap-1.5 mb-2">
            <NodeIcon name="globe" className="w-3 h-3 text-emerald-400" />
            <p className="text-xs font-medium uppercase tracking-wider text-orbflow-text-ghost">Unique Subjects</p>
          </div>
          <p className="text-2xl font-bold text-orbflow-text-secondary tabular-nums">
            {new Set(editedBindings.map((b) => b.subject)).size}
          </p>
        </div>
      </div>

      {/* -- Roles section -- expandable cards --- */}
      <section>
        <div className="flex items-center gap-2 mb-3">
          <NodeIcon name="shield" className="w-4 h-4 text-orbflow-text-ghost" />
          <h3 className="text-heading font-semibold text-orbflow-text-secondary">Roles</h3>
          <button
            type="button"
            onClick={() => setShowRoleForm(true)}
            disabled={showRoleForm}
            className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-body-sm font-medium transition-colors shrink-0
              bg-electric-indigo/10 border border-electric-indigo/20 text-electric-indigo
              hover:bg-electric-indigo/15 hover:border-electric-indigo/30
              disabled:opacity-40 disabled:cursor-not-allowed"
          >
            <NodeIcon name="plus" className="w-3 h-3" />Create Role
          </button>
        </div>

        {/* Role creation form */}
        {showRoleForm && (
          <div className="mb-3">
            <RoleForm
              existingRoleIds={editedRoles.map(r => r.id)}
              onSubmit={handleAddRole}
              onCancel={() => setShowRoleForm(false)}
            />
          </div>
        )}

        {/* Role deletion confirmation */}
        {pendingDeleteRoleId && (() => {
          const roleToDelete = editedRoles.find(r => r.id === pendingDeleteRoleId);
          const affectedBindings = editedBindings.filter(b => b.role_id === pendingDeleteRoleId).length;
          if (!roleToDelete) return null;
          return (
            <div role="alertdialog" aria-label="Confirm role deletion"
              className="mb-3 flex items-center gap-3 px-4 py-3 rounded-xl bg-rose-500/[0.06] border border-rose-500/20 animate-scale-in">
              <NodeIcon name="alert-triangle" className="w-4 h-4 text-rose-400 shrink-0" />
              <p className="text-body-sm text-rose-300 flex-1">
                Delete role <span className="font-semibold text-rose-400">{roleToDelete.name}</span>?
                {affectedBindings > 0 && <> {affectedBindings} binding(s) using this role will also be removed.</>}
              </p>
              <div className="flex items-center gap-2 shrink-0">
                <button type="button" onClick={() => setPendingDeleteRoleId(null)}
                  className="px-3 py-1.5 rounded-lg text-body-sm font-medium text-orbflow-text-ghost hover:text-orbflow-text-secondary transition-colors">
                  Cancel
                </button>
                <button type="button" onClick={() => handleDeleteRole(pendingDeleteRoleId)}
                  className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-body-sm font-medium bg-rose-600 text-white hover:bg-rose-500 transition-colors focus-visible:ring-2 focus-visible:ring-rose-400/50 focus-visible:outline-none">
                  <NodeIcon name="trash" className="w-3 h-3" />Delete
                </button>
              </div>
            </div>
          );
        })()}

        <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
          {editedRoles.map((role) => (
            <RoleCard
              key={role.id}
              role={role}
              bindingCount={bindingCountByRole[role.id] ?? 0}
              onEdit={handleUpdateRole}
              onDelete={() => setPendingDeleteRoleId(role.id)}
            />
          ))}
        </div>
      </section>

      {/* -- Bindings section ------------------ */}
      <section className="rounded-xl border border-orbflow-border bg-orbflow-surface">
        <div className="flex flex-col gap-2 px-4 py-3 border-b border-orbflow-border/50 sm:flex-row sm:items-center sm:px-5 sm:py-3.5">
          <div className="flex items-center gap-2">
            <NodeIcon name="users" className="w-4 h-4 text-orbflow-text-ghost" />
            <h3 className="text-heading font-semibold text-orbflow-text-secondary">
              Bindings
            </h3>
            <span className="text-caption text-orbflow-text-ghost tabular-nums">{editedBindings.length}</span>
          </div>

          {/* Search + Add */}
          <div className="flex items-center gap-2 sm:ml-auto">
            {editedBindings.length > 3 && (
              <div className="relative flex-1 sm:flex-initial">
                <NodeIcon name="search" className="absolute left-2.5 top-1/2 -translate-y-1/2 w-3 h-3 text-orbflow-text-ghost pointer-events-none" />
                <input
                  type="text"
                  value={searchQuery}
                  onChange={(e) => setSearchQuery(e.target.value)}
                  placeholder="Filter bindings..."
                  aria-label="Filter bindings by subject or role"
                  className={cn(
                    "rounded-lg bg-orbflow-surface-hover border border-orbflow-border text-body-sm pl-7 py-1.5 w-full sm:w-48",
                    "placeholder:text-orbflow-text-ghost/50 focus:outline-none focus:ring-1 focus:ring-electric-indigo/40 transition-colors",
                    searchQuery ? "pr-7" : "pr-3"
                  )}
                />
                {searchQuery && (
                  <button
                    type="button"
                    onClick={() => setSearchQuery("")}
                    aria-label="Clear filter"
                    className="absolute right-2 top-1/2 -translate-y-1/2 text-orbflow-text-ghost hover:text-orbflow-text-secondary transition-colors
                      p-0.5 rounded focus-visible:ring-1 focus-visible:ring-electric-indigo/40 focus-visible:outline-none"
                  >
                    <NodeIcon name="x" className="w-3 h-3" />
                  </button>
                )}
              </div>
            )}
            <button
              type="button"
              onClick={() => setShowAddForm(true)}
              disabled={showAddForm}
              className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-body-sm font-medium transition-colors shrink-0
                bg-electric-indigo/10 border border-electric-indigo/20 text-electric-indigo
                hover:bg-electric-indigo/15 hover:border-electric-indigo/30
                disabled:opacity-40 disabled:cursor-not-allowed"
            >
              <NodeIcon name="plus" className="w-3 h-3" />Add Binding
            </button>
          </div>
        </div>

        {/* Add binding form */}
        {showAddForm && (
          <div className="px-4 py-4 border-b border-orbflow-border/30 sm:px-5">
            <AddBindingForm
              roles={editedRoles}
              knownSubjects={knownSubjects}
              onAdd={handleAddBinding}
              onCancel={() => setShowAddForm(false)}
            />
          </div>
        )}

        {/* Remove confirmation inline banner */}
        {pendingRemoveIdx !== null && editedBindings[pendingRemoveIdx] && (
          <div
            role="alertdialog"
            aria-label="Confirm removal"
            className="mx-4 mt-3 sm:mx-5 flex items-center gap-3 px-4 py-3 rounded-xl bg-rose-500/[0.06] border border-rose-500/20 animate-scale-in"
          >
            <NodeIcon name="alert-triangle" className="w-4 h-4 text-rose-400 shrink-0" />
            <p className="text-body-sm text-rose-300 flex-1">
              Remove binding for <span className="font-semibold text-rose-400">{editedBindings[pendingRemoveIdx].subject}</span>?
            </p>
            <div className="flex items-center gap-2 shrink-0">
              <button
                type="button"
                onClick={() => setPendingRemoveIdx(null)}
                className="px-3 py-1.5 rounded-lg text-body-sm font-medium text-orbflow-text-ghost hover:text-orbflow-text-secondary transition-colors"
              >
                Cancel
              </button>
              <button
                type="button"
                onClick={() => handleRemoveBinding(pendingRemoveIdx)}
                className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-body-sm font-medium
                  bg-rose-600 text-white hover:bg-rose-500 transition-colors
                  focus-visible:ring-2 focus-visible:ring-rose-400/50 focus-visible:outline-none"
              >
                <NodeIcon name="trash" className="w-3 h-3" />Remove
              </button>
            </div>
          </div>
        )}

        {/* Bindings table / cards */}
        {editedBindings.length === 0 && !showAddForm ? (
          <div className="flex flex-col items-center justify-center py-16 text-orbflow-text-ghost">
            <div className="w-14 h-14 rounded-2xl bg-orbflow-surface-hover/60 flex items-center justify-center mb-3">
              <NodeIcon name="users" className="w-7 h-7 opacity-40" />
            </div>
            <p className="text-body font-medium text-orbflow-text-muted">No bindings configured</p>
            <p className="text-body-sm text-orbflow-text-ghost mt-1 mb-4">Add a binding to grant users access to your workspace</p>
            <button
              type="button"
              onClick={() => setShowAddForm(true)}
              className="flex items-center gap-1.5 px-4 py-2 rounded-lg text-body-sm font-medium transition-colors
                bg-electric-indigo/10 border border-electric-indigo/20 text-electric-indigo
                hover:bg-electric-indigo/15 hover:border-electric-indigo/30"
            >
              <NodeIcon name="plus" className="w-3 h-3" />Add First Binding
            </button>
          </div>
        ) : filteredBindings.length > 0 ? (
          <div className="overflow-x-auto">
            <table className="w-full text-left hidden sm:table">
              <thead>
                <tr className="border-b border-orbflow-border/50">
                  <th className="text-caption uppercase tracking-wider font-semibold text-orbflow-text-ghost px-4 py-2.5">Subject</th>
                  <th className="text-caption uppercase tracking-wider font-semibold text-orbflow-text-ghost px-4 py-2.5">Role</th>
                  <th className="text-caption uppercase tracking-wider font-semibold text-orbflow-text-ghost px-4 py-2.5">Scope</th>
                  <th className="text-caption uppercase tracking-wider font-semibold text-orbflow-text-ghost px-4 py-2.5 w-16" />
                </tr>
              </thead>
              <tbody>
                {filteredBindings.map((binding, idx) => {
                  const originalIdx = editedBindings.indexOf(binding);
                  return (
                    <BindingTableRow
                      key={`${binding.subject}-${binding.role_id}-${idx}`}
                      binding={binding}
                      roleName={roleNameMap.get(binding.role_id) ?? binding.role_id}
                      onRemove={() => setPendingRemoveIdx(originalIdx)}
                    />
                  );
                })}
              </tbody>
            </table>
            {/* Mobile card list */}
            <div className="sm:hidden">
              {filteredBindings.map((binding, idx) => {
                const originalIdx = editedBindings.indexOf(binding);
                return (
                  <BindingMobileCard
                    key={`m-${binding.subject}-${binding.role_id}-${idx}`}
                    binding={binding}
                    roleName={roleNameMap.get(binding.role_id) ?? binding.role_id}
                    onRemove={() => setPendingRemoveIdx(originalIdx)}
                  />
                );
              })}
            </div>
          </div>
        ) : searchQuery && (
          <div className="py-10 text-center">
            <NodeIcon name="search" className="w-5 h-5 text-orbflow-text-ghost/40 mx-auto mb-2" />
            <p className="text-sm text-orbflow-text-ghost">
              No bindings match &quot;{searchQuery}&quot;
            </p>
            <button
              type="button"
              onClick={() => setSearchQuery("")}
              className="text-sm text-electric-indigo hover:text-electric-indigo/80 mt-1 transition-colors"
            >
              Clear filter
            </button>
          </div>
        )}
      </section>

      {/* -- Action bar (save / discard) -------- */}
      {isDirty && (
        <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between pt-2 sticky bottom-0 bg-orbflow-bg/80 backdrop-blur-sm pb-4 -mx-4 px-4 sm:-mx-6 sm:px-6 border-t border-orbflow-border/50">
          <p className="text-body-sm text-orbflow-text-ghost">
            {(() => {
              const rolesDelta = editedRoles.length - (policy?.roles.length ?? 0);
              const bindingsDelta = editedBindings.length - (policy?.bindings.length ?? 0);
              const parts: string[] = [];
              if (rolesDelta > 0) parts.push(`${rolesDelta} role(s) added`);
              if (rolesDelta < 0) parts.push(`${Math.abs(rolesDelta)} role(s) removed`);
              if (bindingsDelta > 0) parts.push(`${bindingsDelta} binding(s) added`);
              if (bindingsDelta < 0) parts.push(`${Math.abs(bindingsDelta)} binding(s) removed`);
              if (parts.length === 0) parts.push("Changes pending");
              return parts.join(", ");
            })()}
          </p>
          <div className="flex items-center gap-3">
            <button
              type="button"
              onClick={handleDiscard}
              className="flex items-center gap-1.5 px-4 py-2 rounded-lg text-body font-medium transition-colors
                bg-orbflow-surface-hover/50 border border-orbflow-border text-orbflow-text-faint
                hover:text-orbflow-text-secondary hover:border-orbflow-border-hover"
            >
              <NodeIcon name="x" className="w-3.5 h-3.5" />Discard
            </button>
            <button
              type="button"
              onClick={handleSave}
              disabled={saving || validationError !== null}
              className="flex items-center gap-1.5 px-5 py-2 rounded-lg text-body font-semibold transition-colors
                bg-emerald-600 text-white hover:bg-emerald-500
                disabled:opacity-40 disabled:cursor-not-allowed disabled:hover:bg-emerald-600
                focus-visible:ring-2 focus-visible:ring-emerald-400/50 focus-visible:outline-none"
            >
              {saving ? (
                <>
                  <div className="w-3.5 h-3.5 animate-spin rounded-full border-2 border-white/30 border-t-white" />
                  Saving...
                </>
              ) : (
                <>
                  <NodeIcon name="check" className="w-3.5 h-3.5" />Save Changes
                </>
              )}
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
