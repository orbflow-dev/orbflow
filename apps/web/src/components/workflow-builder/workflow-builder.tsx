"use client";

import { OrbflowWorkflowBuilderInner } from "@/core/components/orbflow-workflow-builder";
import { WorkflowSelector } from "./workflow-selector";
import { useWorkflowIo } from "./use-workflow-io";

export function WorkflowBuilder() {
  const {
    selectedWorkflow,
    workflows,
    defaultName,
    fileInputRef,
    handleSelect,
    handleImport,
    handleExport,
    handleFileChange,
  } = useWorkflowIo();

  return (
    <div className="flex h-full relative">
      <WorkflowSelector
        workflows={workflows}
        selectedWorkflow={selectedWorkflow}
        onSelect={handleSelect}
        onImport={handleImport}
        onExport={handleExport}
      />

      {/* Hidden file input for import */}
      <input
        ref={fileInputRef}
        type="file"
        accept=".json"
        className="hidden"
        onChange={handleFileChange}
      />

      {/* Visual Builder */}
      <OrbflowWorkflowBuilderInner
        key={selectedWorkflow?.id ?? "new"}
        workflow={selectedWorkflow || undefined}
        defaultName={defaultName}
      />
    </div>
  );
}
