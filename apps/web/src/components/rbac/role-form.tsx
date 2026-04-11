"use client";

import { useState } from "react";
import { NodeIcon } from "@/core/components/icons";
import { cn } from "@/lib/cn";
import type { Role, Permission } from "@orbflow/core";

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

/* =======================================================
   Helpers
   ======================================================= */

function toKebabCase(name: string): string {
  return name.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "").slice(0, 64);
}

/* =======================================================
   RoleForm
   ======================================================= */

interface RoleFormProps {
  readonly existingRoleIds: string[];
  readonly initialRole?: Role;
  readonly onSubmit: (role: Role) => void;
  readonly onCancel: () => void;
}

export function RoleForm({ existingRoleIds, initialRole, onSubmit, onCancel }: RoleFormProps) {
  const isEditing = initialRole !== undefined;

  const [name, setName] = useState(initialRole?.name ?? "");
  const [id, setId] = useState(initialRole?.id ?? "");
  const [idManuallyEdited, setIdManuallyEdited] = useState(isEditing);
  const [description, setDescription] = useState(initialRole?.description ?? "");
  const [permissions, setPermissions] = useState<Permission[]>(
    initialRole ? [...initialRole.permissions] : []
  );

  const inputClasses =
    "w-full rounded-lg bg-orbflow-surface border border-orbflow-border text-orbflow-text-secondary text-body-sm px-3 py-2 " +
    "placeholder:text-orbflow-text-ghost/50 focus:outline-none focus:ring-2 focus:ring-electric-indigo/50 focus:border-electric-indigo/40 transition-colors";

  function handleNameChange(value: string) {
    setName(value);
    if (!idManuallyEdited) {
      setId(toKebabCase(value));
    }
  }

  function handleIdChange(value: string) {
    const sanitized = value.toLowerCase().replace(/[^a-z0-9-]/g, "").slice(0, 64);
    setId(sanitized);
    setIdManuallyEdited(true);
  }

  function togglePermission(perm: Permission) {
    setPermissions((prev) =>
      prev.includes(perm) ? prev.filter((p) => p !== perm) : [...prev, perm]
    );
  }

  const nameValid = name.trim().length >= 3 && name.trim().length <= 64;
  const idValid = id.length >= 1 && /^[a-z0-9]([a-z0-9-]*[a-z0-9])?$/.test(id);
  const idUnique = isEditing || !existingRoleIds.includes(id);
  const hasPermissions = permissions.length > 0;
  const canSubmit = nameValid && idValid && idUnique && hasPermissions;

  function handleSubmit() {
    if (!canSubmit) return;
    onSubmit({
      id,
      name: name.trim(),
      description: description.trim(),
      permissions,
      builtin: false,
    });
  }

  return (
    <div className="rounded-xl border border-electric-indigo/20 bg-electric-indigo/[0.02] p-4 space-y-4 animate-scale-in">
      <h4 className="text-body font-semibold text-orbflow-text-secondary">
        {isEditing ? "Edit Role" : "Create Role"}
      </h4>

      <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
        {/* Name */}
        <div className="space-y-1.5">
          <label htmlFor="role-name" className="text-caption uppercase tracking-wider font-semibold text-orbflow-text-ghost">
            Name
          </label>
          <input
            id="role-name"
            type="text"
            value={name}
            onChange={(e) => handleNameChange(e.target.value)}
            placeholder="e.g. Deploy Manager"
            className={inputClasses}
            autoFocus
            onKeyDown={(e) => {
              if (e.key === "Enter") handleSubmit();
              if (e.key === "Escape") onCancel();
            }}
          />
          {name.length > 0 && !nameValid && (
            <p className="text-micro text-rose-400">Name must be 3-64 characters</p>
          )}
        </div>

        {/* ID */}
        <div className="space-y-1.5">
          <label htmlFor="role-id" className="text-caption uppercase tracking-wider font-semibold text-orbflow-text-ghost">
            ID
          </label>
          <input
            id="role-id"
            type="text"
            value={id}
            onChange={(e) => handleIdChange(e.target.value)}
            placeholder="auto-generated-from-name"
            disabled={isEditing}
            className={cn(inputClasses, isEditing && "opacity-50 cursor-not-allowed")}
            onKeyDown={(e) => {
              if (e.key === "Enter") handleSubmit();
              if (e.key === "Escape") onCancel();
            }}
          />
          {id.length > 0 && !idValid && (
            <p className="text-micro text-rose-400">Only lowercase letters, numbers, and hyphens</p>
          )}
          {id.length > 0 && idValid && !idUnique && (
            <p className="text-micro text-rose-400">Role ID already exists</p>
          )}
        </div>
      </div>

      {/* Description */}
      <div className="space-y-1.5">
        <label htmlFor="role-description" className="text-caption uppercase tracking-wider font-semibold text-orbflow-text-ghost">
          Description (optional)
        </label>
        <input
          id="role-description"
          type="text"
          value={description}
          onChange={(e) => setDescription(e.target.value)}
          placeholder="What this role is for..."
          className={inputClasses}
          onKeyDown={(e) => {
            if (e.key === "Enter") handleSubmit();
            if (e.key === "Escape") onCancel();
          }}
        />
      </div>

      {/* Permissions */}
      <div className="space-y-1.5">
        <label className="text-caption uppercase tracking-wider font-semibold text-orbflow-text-ghost">
          Permissions
        </label>
        <div className="flex flex-wrap gap-1.5">
          {ALL_PERMISSIONS.map((perm) => {
            const selected = permissions.includes(perm);
            const colors = PERMISSION_COLORS[perm];
            return (
              <button
                key={perm}
                type="button"
                onClick={() => togglePermission(perm)}
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
        {permissions.length === 0 && (
          <p className="text-micro text-orbflow-text-ghost">Select at least one permission</p>
        )}
      </div>

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
          <NodeIcon name="check" className="w-3 h-3" />
          {isEditing ? "Update Role" : "Create Role"}
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
