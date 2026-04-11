"use client";

import { NodeIcon } from "../icons";
import type { QuickTemplate } from "./quick-templates";

interface WelcomeOverlayProps {
  templates: QuickTemplate[];
  onSelectTemplate: (template: QuickTemplate) => void;
  onOpenPicker: () => void;
}

export function WelcomeOverlay({
  templates,
  onSelectTemplate,
  onOpenPicker,
}: WelcomeOverlayProps) {
  return (
    <div className="absolute inset-0 flex items-center justify-center z-10 pointer-events-none">
      <div className="pointer-events-auto max-w-lg w-full px-4">
        <div className="text-center mb-8 animate-fade-in-up">
          <div className="inline-flex items-center justify-center w-16 h-16 rounded-2xl bg-electric-indigo/10 border border-electric-indigo/20 mb-5 animate-float">
            <NodeIcon
              name="workflow"
              className="w-8 h-8 text-electric-indigo drop-shadow-[0_0_8px_rgba(124,92,252,0.3)]"
            />
          </div>
          <h2 className="text-2xl font-bold mb-2 tracking-tight text-orbflow-text-secondary">
            What would you like to automate?
          </h2>
          <p className="text-sm max-w-sm mx-auto leading-relaxed text-orbflow-text-faint">
            Pick a template to get started, or press + to add steps
          </p>
        </div>

        <div className="grid grid-cols-2 gap-3 animate-fade-in-up stagger-2">
          {templates.map((t) => (
            <button
              key={t.id}
              onClick={() => onSelectTemplate(t)}
              className="group text-left p-4 rounded-xl backdrop-blur-sm
                hover:bg-orbflow-surface-hover transition-all duration-300
                active:brightness-95 border border-orbflow-border bg-orbflow-glass-bg
                focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none"
            >
              <div className="flex items-start gap-3">
                <div
                  className="w-8 h-8 rounded-lg flex items-center justify-center shrink-0 transition-all duration-300 group-hover:brightness-110 group-hover:shadow-md"
                  style={{ backgroundColor: t.color + "15" }}
                >
                  <NodeIcon
                    name={t.icon}
                    className="w-4 h-4"
                    style={{ color: t.color }}
                  />
                </div>
                <div className="min-w-0">
                  <div className="text-heading font-semibold mb-0.5 text-orbflow-text-secondary">
                    {t.name}
                  </div>
                  <div className="text-body-sm leading-relaxed text-orbflow-text-faint">
                    {t.description}
                  </div>
                </div>
              </div>
              <div className="mt-3 flex items-center gap-1">
                {t.nodes.map((n, i) => (
                  <div key={n.id} className="flex items-center">
                    <span className="text-caption font-mono px-1.5 py-0.5 rounded text-orbflow-text-faint bg-orbflow-add-btn-bg">
                      {(n.data.label as string).split(" ")[0]}
                    </span>
                    {i < t.nodes.length - 1 && (
                      <NodeIcon
                        name="arrow-right"
                        className="w-3 h-3 mx-0.5 text-orbflow-text-ghost"
                      />
                    )}
                  </div>
                ))}
              </div>
            </button>
          ))}
        </div>

        <div className="text-center mt-6 animate-fade-in stagger-4">
          <button
            onClick={onOpenPicker}
            className="mx-auto mb-4 w-12 h-12 rounded-full flex items-center justify-center
              bg-electric-indigo/15 border border-electric-indigo/25
              text-electric-indigo hover:bg-electric-indigo/25 hover:border-electric-indigo/40
              transition-all duration-200 active:brightness-95 shadow-lg shadow-electric-indigo/10
              focus-visible:ring-2 focus-visible:ring-electric-indigo/50 focus-visible:outline-none"
            title="Add your first step"
            aria-label="Add your first step"
          >
            <NodeIcon name="plus" className="w-6 h-6" />
          </button>
          <div className="flex items-center justify-center gap-4 text-body-sm text-orbflow-text-ghost">
            <span className="uppercase tracking-widest">
              or pick a template above
            </span>
          </div>
          <div className="flex justify-center gap-6 mt-3 text-caption font-mono text-orbflow-text-ghost">
            <span>Ctrl+Z undo</span>
            <span>Ctrl+S save</span>
            <span>Ctrl+Enter run</span>
            <span>Del delete</span>
          </div>
        </div>
      </div>
    </div>
  );
}
