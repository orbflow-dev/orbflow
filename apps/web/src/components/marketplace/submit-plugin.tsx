"use client";

import { useState, useCallback } from "react";
import { createPortal } from "react-dom";
import { NodeIcon } from "@/core";
import { api } from "@/lib/api";
import { cn } from "@/lib/cn";

const CATEGORIES = [
  { value: "", label: "None" },
  { value: "ai", label: "AI" },
  { value: "database", label: "Database" },
  { value: "communication", label: "Communication" },
  { value: "utility", label: "Utility" },
  { value: "monitoring", label: "Monitoring" },
  { value: "security", label: "Security" },
  { value: "cloud", label: "Cloud" },
  { value: "integration", label: "Integration" },
] as const;

interface SubmitPluginProps {
  readonly onClose: () => void;
}

type Step = "prerequisites" | "form" | "submit";

interface ManifestForm {
  name: string;
  version: string;
  description: string;
  author: string;
  category: string;
  tags: string;
  icon: string;
  color: string;
  repo: string;
  path: string;
  language: string;
  protocol: string;
  node_types: string;
  license: string;
  orbflow_version: string;
}

const INITIAL_FORM: ManifestForm = {
  name: "",
  version: "0.1.0",
  description: "",
  author: "",
  category: "",
  tags: "",
  icon: "",
  color: "",
  repo: "",
  path: "",
  language: "python",
  protocol: "grpc",
  node_types: "",
  license: "MIT",
  orbflow_version: ">=0.1.0",
};

export function SubmitPlugin({ onClose }: SubmitPluginProps) {
  const [step, setStep] = useState<Step>("prerequisites");
  const [form, setForm] = useState<ManifestForm>(INITIAL_FORM);
  const [validationErrors, setValidationErrors] = useState<string[]>([]);
  const [validating, setValidating] = useState(false);
  const [copied, setCopied] = useState(false);

  const updateField = useCallback((field: keyof ManifestForm, value: string) => {
    setForm((prev) => ({ ...prev, [field]: value }));
  }, []);

  const buildEntry = useCallback((): Record<string, unknown> => {
    const entry: Record<string, unknown> = {
      name: form.name.trim(),
      version: form.version.trim(),
      description: form.description.trim(),
      author: form.author.trim(),
      protocol: form.protocol,
      node_types: form.node_types
        .split(",")
        .map((s) => s.trim())
        .filter(Boolean),
      license: form.license.trim(),
      orbflow_version: form.orbflow_version.trim(),
      tags: form.tags
        .split(",")
        .map((s) => s.trim())
        .filter(Boolean),
      downloads: 0,
    };
    if (form.category) entry.category = form.category;
    if (form.icon.trim()) entry.icon = form.icon.trim();
    if (form.color.trim()) entry.color = form.color.trim();
    if (form.repo.trim()) entry.repo = form.repo.trim();
    if (form.path.trim()) entry.path = form.path.trim();
    if (form.language.trim()) entry.language = form.language.trim();
    return entry;
  }, [form]);

  const handleValidate = useCallback(async () => {
    setValidating(true);
    setValidationErrors([]);
    try {
      const entry = buildEntry();
      const result = await api.marketplace.validateManifest(entry);
      if (result.valid) {
        setStep("submit");
      } else {
        setValidationErrors(result.errors ?? ["Validation failed"]);
      }
    } catch (err: unknown) {
      setValidationErrors([err instanceof Error ? err.message : "Validation request failed"]);
    } finally {
      setValidating(false);
    }
  }, [buildEntry]);

  const handleCopy = useCallback(async () => {
    const entry = buildEntry();
    const json = JSON.stringify(entry, null, 2);
    try {
      await navigator.clipboard.writeText(json);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      // Fallback: select text in the pre element
    }
  }, [buildEntry]);

  const handleBackdropClick = useCallback(
    (e: React.MouseEvent) => {
      if (e.target === e.currentTarget) onClose();
    },
    [onClose],
  );

  return createPortal(
    <div
      role="dialog"
      aria-modal="true"
      aria-label="Submit Plugin"
      onClick={handleBackdropClick}
      className="fixed inset-0 z-[85] flex items-center justify-center bg-black/50 backdrop-blur-sm
        animate-[fadeIn_150ms_ease-out]"
    >
      <div className="w-full max-w-2xl max-h-[85vh] bg-orbflow-bg border border-orbflow-border rounded-2xl
        overflow-hidden flex flex-col shadow-2xl shadow-black/40
        animate-[modalSlideUp_300ms_cubic-bezier(0.16,1,0.3,1)]">
        {/* Header */}
        <div className="flex items-center justify-between p-5 border-b border-orbflow-border shrink-0">
          <div className="flex items-center gap-3">
            <div className="w-9 h-9 rounded-xl bg-gradient-to-br from-electric-indigo to-electric-indigo/60
              flex items-center justify-center">
              <NodeIcon name="upload" className="w-4.5 h-4.5 text-white" />
            </div>
            <div>
              <h2 className="text-sm font-bold text-orbflow-text-secondary">Submit a Plugin</h2>
              <p className="text-[11px] text-orbflow-text-ghost mt-0.5">
                {step === "prerequisites" && "Step 1 of 3 — Prerequisites"}
                {step === "form" && "Step 2 of 3 — Plugin Details"}
                {step === "submit" && "Step 3 of 3 — Submit to Registry"}
              </p>
            </div>
          </div>
          <button
            type="button"
            onClick={onClose}
            className="w-8 h-8 rounded-lg flex items-center justify-center
              hover:bg-orbflow-surface-hover transition-colors"
            aria-label="Close"
          >
            <NodeIcon name="x" className="w-4 h-4 text-orbflow-text-muted" />
          </button>
        </div>

        {/* Step indicator */}
        <div className="flex items-center gap-1 px-5 pt-4">
          {(["prerequisites", "form", "submit"] as const).map((s, i) => (
            <div
              key={s}
              className={cn(
                "h-1 flex-1 rounded-full transition-all duration-300",
                step === s || (s === "prerequisites" && step !== "prerequisites") || (s === "form" && step === "submit")
                  ? "bg-electric-indigo"
                  : "bg-orbflow-surface-hover",
              )}
            />
          ))}
        </div>

        {/* Body */}
        <div className="flex-1 overflow-y-auto p-5">
          {step === "prerequisites" && (
            <PrerequisitesStep onNext={() => setStep("form")} />
          )}
          {step === "form" && (
            <FormStep
              form={form}
              onUpdate={updateField}
              errors={validationErrors}
              validating={validating}
              onValidate={handleValidate}
              onBack={() => setStep("prerequisites")}
            />
          )}
          {step === "submit" && (
            <SubmitStep
              entry={buildEntry()}
              copied={copied}
              onCopy={handleCopy}
              onBack={() => setStep("form")}
              onClose={onClose}
            />
          )}
        </div>
      </div>
    </div>,
    document.body,
  );
}

function PrerequisitesStep({ onNext }: { readonly onNext: () => void }) {
  const checks = [
    { icon: "github" as const, label: "Plugin hosted on a public GitHub repository" },
    { icon: "file-text" as const, label: "Repository contains an orbflow-plugin.json manifest" },
    { icon: "package" as const, label: "Plugin implements the Orbflow plugin protocol (gRPC or Subprocess)" },
    { icon: "code" as const, label: "Plugin is tested and working locally" },
  ];

  return (
    <div className="space-y-6">
      <div>
        <h3 className="text-sm font-semibold text-orbflow-text-secondary mb-2">Before you submit</h3>
        <p className="text-xs text-orbflow-text-muted leading-relaxed">
          Make sure your plugin meets these requirements before submitting to the community registry.
        </p>
      </div>

      <div className="space-y-3">
        {checks.map((check) => (
          <div
            key={check.label}
            className="flex items-start gap-3 rounded-xl bg-orbflow-surface border border-orbflow-border/50 p-4"
          >
            <div className="w-8 h-8 rounded-lg bg-electric-indigo/10 flex items-center justify-center shrink-0 mt-0.5">
              <NodeIcon name={check.icon} className="w-4 h-4 text-electric-indigo" />
            </div>
            <p className="text-sm text-orbflow-text-muted leading-relaxed pt-1">{check.label}</p>
          </div>
        ))}
      </div>

      <div className="rounded-xl bg-orbflow-surface border border-orbflow-border/50 p-4">
        <h4 className="text-xs font-semibold text-orbflow-text-ghost uppercase tracking-wider mb-2">Resources</h4>
        <div className="space-y-2">
          <a
            href="https://github.com/orbflow-dev/orbflow-plugins"
            target="_blank"
            rel="noopener noreferrer"
            className="flex items-center gap-2 text-xs text-electric-indigo hover:text-electric-indigo/80 transition-colors"
          >
            <NodeIcon name="link" className="w-3.5 h-3.5" />
            Community Plugin Registry
          </a>
          <a
            href="https://github.com/orbflow-dev/orbflow/tree/main/docs/plugins"
            target="_blank"
            rel="noopener noreferrer"
            className="flex items-center gap-2 text-xs text-electric-indigo hover:text-electric-indigo/80 transition-colors"
          >
            <NodeIcon name="link" className="w-3.5 h-3.5" />
            Plugin Development Guide
          </a>
        </div>
      </div>

      <button
        type="button"
        onClick={onNext}
        className="w-full rounded-xl bg-electric-indigo text-white py-3 text-sm font-semibold
          flex items-center justify-center gap-2 shadow-md shadow-electric-indigo/20
          hover:shadow-lg hover:brightness-110 transition-all duration-200"
      >
        I meet the requirements
        <NodeIcon name="arrow-right" className="w-4 h-4" />
      </button>
    </div>
  );
}

function FormStep({
  form,
  onUpdate,
  errors,
  validating,
  onValidate,
  onBack,
}: {
  readonly form: ManifestForm;
  readonly onUpdate: (field: keyof ManifestForm, value: string) => void;
  readonly errors: string[];
  readonly validating: boolean;
  readonly onValidate: () => void;
  readonly onBack: () => void;
}) {
  return (
    <div className="space-y-5">
      <div>
        <h3 className="text-sm font-semibold text-orbflow-text-secondary mb-1">Plugin Details</h3>
        <p className="text-xs text-orbflow-text-muted leading-relaxed">
          Fill in your plugin information. This will generate the JSON entry for the community registry.
        </p>
      </div>

      <div className="grid grid-cols-2 gap-3">
        <Field label="Plugin Name *" placeholder="orbflow-my-plugin" value={form.name}
          onChange={(v) => onUpdate("name", v)} />
        <Field label="Version *" placeholder="0.1.0" value={form.version}
          onChange={(v) => onUpdate("version", v)} />
        <div className="col-span-2">
          <Field label="Description *" placeholder="What does your plugin do?" value={form.description}
            onChange={(v) => onUpdate("description", v)} />
        </div>
        <Field label="Author *" placeholder="Your name or org" value={form.author}
          onChange={(v) => onUpdate("author", v)} />
        <SelectField label="Category" value={form.category}
          options={CATEGORIES.map((c) => ({ value: c.value, label: c.label }))}
          onChange={(v) => onUpdate("category", v)} />
        <Field label="Node Types *" placeholder="plugin:my-action, plugin:my-trigger" value={form.node_types}
          onChange={(v) => onUpdate("node_types", v)} hint="Comma-separated" />
        <SelectField label="Protocol *" value={form.protocol}
          options={[{ value: "grpc", label: "gRPC" }, { value: "subprocess", label: "Subprocess" }]}
          onChange={(v) => onUpdate("protocol", v)} />
        <Field label="GitHub Repo" placeholder="owner/repo-name" value={form.repo}
          onChange={(v) => onUpdate("repo", v)} hint="For downloads" />
        <Field label="Path in Repo" placeholder="python/orbflow/my-plugin" value={form.path}
          onChange={(v) => onUpdate("path", v)} hint="Monorepo path" />
        <Field label="Tags" placeholder="csv, parser, data" value={form.tags}
          onChange={(v) => onUpdate("tags", v)} hint="Comma-separated" />
        <SelectField label="Language" value={form.language}
          options={[
            { value: "python", label: "Python" },
            { value: "typescript", label: "TypeScript" },
            { value: "rust", label: "Rust" },
            { value: "go", label: "Go" },
          ]}
          onChange={(v) => onUpdate("language", v)} />
        <Field label="License" placeholder="MIT" value={form.license}
          onChange={(v) => onUpdate("license", v)} />
        <Field label="Min Orbflow Version" placeholder=">=0.1.0" value={form.orbflow_version}
          onChange={(v) => onUpdate("orbflow_version", v)} />
        <Field label="Icon" placeholder="terminal" value={form.icon}
          onChange={(v) => onUpdate("icon", v)} hint="Icon name" />
        <Field label="Color" placeholder="#6366F1" value={form.color}
          onChange={(v) => onUpdate("color", v)} hint="Hex color" />
      </div>

      {errors.length > 0 && (
        <div className="rounded-xl border border-rose-500/30 bg-rose-500/5 p-4 space-y-1.5">
          <p className="text-xs font-semibold text-rose-400">Validation Errors</p>
          {errors.map((err) => (
            <p key={err} className="text-xs text-rose-300 flex items-start gap-2">
              <NodeIcon name="x" className="w-3 h-3 mt-0.5 shrink-0" />
              {err}
            </p>
          ))}
        </div>
      )}

      <div className="flex items-center gap-3">
        <button
          type="button"
          onClick={onBack}
          className="flex-1 rounded-xl border border-orbflow-border py-3 text-sm font-medium
            text-orbflow-text-muted hover:bg-orbflow-surface-hover transition-all duration-200"
        >
          Back
        </button>
        <button
          type="button"
          onClick={onValidate}
          disabled={validating}
          className="flex-1 rounded-xl bg-electric-indigo text-white py-3 text-sm font-semibold
            flex items-center justify-center gap-2 shadow-md shadow-electric-indigo/20
            hover:shadow-lg hover:brightness-110 transition-all duration-200
            disabled:opacity-50 disabled:cursor-not-allowed"
        >
          {validating ? (
            <>
              <NodeIcon name="loader" className="w-4 h-4 animate-spin" />
              Validating...
            </>
          ) : (
            <>
              Validate & Continue
              <NodeIcon name="arrow-right" className="w-4 h-4" />
            </>
          )}
        </button>
      </div>
    </div>
  );
}

function SubmitStep({
  entry,
  copied,
  onCopy,
  onBack,
  onClose,
}: {
  readonly entry: Record<string, unknown>;
  readonly copied: boolean;
  readonly onCopy: () => void;
  readonly onBack: () => void;
  readonly onClose: () => void;
}) {
  const json = JSON.stringify(entry, null, 2);
  const editUrl = "https://github.com/orbflow-dev/orbflow-plugins/edit/master/plugins.json";

  return (
    <div className="space-y-5">
      <div>
        <h3 className="text-sm font-semibold text-orbflow-text-secondary mb-1">Submit to Registry</h3>
        <p className="text-xs text-orbflow-text-muted leading-relaxed">
          Copy the JSON entry below and add it to the <code className="font-mono text-electric-indigo">plugins.json</code> file
          in the community registry via a GitHub Pull Request.
        </p>
      </div>

      {/* JSON preview */}
      <div className="relative">
        <pre className="rounded-xl border border-orbflow-border bg-orbflow-surface p-4
          text-xs font-mono text-orbflow-text-muted leading-relaxed overflow-x-auto
          max-h-64 scrollbar-thin">
          {json}
        </pre>
        <button
          type="button"
          onClick={onCopy}
          className={cn(
            "absolute top-3 right-3 rounded-lg px-3 py-1.5 text-xs font-medium transition-all duration-200",
            "flex items-center gap-1.5",
            copied
              ? "bg-emerald-500/15 text-emerald-400 ring-1 ring-emerald-500/20"
              : "bg-orbflow-surface-hover text-orbflow-text-muted hover:text-orbflow-text-secondary ring-1 ring-orbflow-border/30",
          )}
        >
          <NodeIcon name={copied ? "check" : "clipboard"} className="w-3.5 h-3.5" />
          {copied ? "Copied!" : "Copy"}
        </button>
      </div>

      {/* Instructions */}
      <div className="rounded-xl bg-orbflow-surface border border-orbflow-border/50 p-4 space-y-3">
        <h4 className="text-xs font-semibold text-orbflow-text-ghost uppercase tracking-wider">How to submit</h4>
        <ol className="space-y-2 text-xs text-orbflow-text-muted leading-relaxed">
          <li className="flex items-start gap-2">
            <span className="shrink-0 w-5 h-5 rounded-md bg-electric-indigo/10 text-electric-indigo flex items-center justify-center text-[10px] font-bold">1</span>
            Copy the JSON entry above
          </li>
          <li className="flex items-start gap-2">
            <span className="shrink-0 w-5 h-5 rounded-md bg-electric-indigo/10 text-electric-indigo flex items-center justify-center text-[10px] font-bold">2</span>
            Click &ldquo;Open on GitHub&rdquo; below to edit <code className="font-mono">plugins.json</code>
          </li>
          <li className="flex items-start gap-2">
            <span className="shrink-0 w-5 h-5 rounded-md bg-electric-indigo/10 text-electric-indigo flex items-center justify-center text-[10px] font-bold">3</span>
            Add your entry to the array and submit a Pull Request
          </li>
          <li className="flex items-start gap-2">
            <span className="shrink-0 w-5 h-5 rounded-md bg-electric-indigo/10 text-electric-indigo flex items-center justify-center text-[10px] font-bold">4</span>
            Your plugin will appear in the marketplace after the PR is merged
          </li>
        </ol>
      </div>

      <div className="flex items-center gap-3">
        <button
          type="button"
          onClick={onBack}
          className="flex-1 rounded-xl border border-orbflow-border py-3 text-sm font-medium
            text-orbflow-text-muted hover:bg-orbflow-surface-hover transition-all duration-200"
        >
          Back
        </button>
        <a
          href={editUrl}
          target="_blank"
          rel="noopener noreferrer"
          className="flex-1 rounded-xl bg-electric-indigo text-white py-3 text-sm font-semibold
            flex items-center justify-center gap-2 shadow-md shadow-electric-indigo/20
            hover:shadow-lg hover:brightness-110 transition-all duration-200"
        >
          <NodeIcon name="link" className="w-4 h-4" />
          Open on GitHub
        </a>
      </div>

      <button
        type="button"
        onClick={onClose}
        className="w-full text-center text-xs text-orbflow-text-ghost hover:text-orbflow-text-muted transition-colors py-2"
      >
        Done
      </button>
    </div>
  );
}

function Field({
  label,
  placeholder,
  value,
  onChange,
  hint,
}: {
  readonly label: string;
  readonly placeholder: string;
  readonly value: string;
  readonly onChange: (value: string) => void;
  readonly hint?: string;
}) {
  return (
    <div>
      <label className="block text-[11px] font-semibold text-orbflow-text-ghost uppercase tracking-wider mb-1.5">
        {label}
      </label>
      <input
        type="text"
        placeholder={placeholder}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        className="w-full rounded-lg border border-orbflow-border/50 bg-orbflow-surface
          px-3 py-2 text-sm text-orbflow-text-secondary placeholder:text-orbflow-text-ghost/40
          focus:outline-none focus:ring-2 focus:ring-electric-indigo/40 focus:border-electric-indigo/40
          transition-all duration-200"
      />
      {hint && <p className="text-[10px] text-orbflow-text-ghost/50 mt-1">{hint}</p>}
    </div>
  );
}

function SelectField({
  label,
  value,
  options,
  onChange,
}: {
  readonly label: string;
  readonly value: string;
  readonly options: readonly { readonly value: string; readonly label: string }[];
  readonly onChange: (value: string) => void;
}) {
  return (
    <div>
      <label className="block text-[11px] font-semibold text-orbflow-text-ghost uppercase tracking-wider mb-1.5">
        {label}
      </label>
      <select
        value={value}
        onChange={(e) => onChange(e.target.value)}
        className="w-full rounded-lg border border-orbflow-border/50 bg-orbflow-surface
          px-3 py-2 text-sm text-orbflow-text-secondary
          focus:outline-none focus:ring-2 focus:ring-electric-indigo/40
          transition-all duration-200 cursor-pointer"
      >
        {options.map((opt) => (
          <option key={opt.value} value={opt.value}>{opt.label}</option>
        ))}
      </select>
    </div>
  );
}
