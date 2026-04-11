import type { ExecutionStatus } from "../../execution/execution-status";

export type EdgeExecutionStatus = "idle" | "active" | "completed" | "failed";

/** Derive edge visual status from source/target node execution statuses. */
export function deriveEdgeStatus(
  sourceStatus: ExecutionStatus | undefined,
  targetStatus: ExecutionStatus | undefined,
): EdgeExecutionStatus {
  if (
    sourceStatus === "completed" &&
    (targetStatus === "running" || targetStatus === "queued")
  ) {
    return "active";
  }
  if (sourceStatus === "completed" && targetStatus === "completed") {
    return "completed";
  }
  if (targetStatus === "failed") {
    return "failed";
  }
  return "idle";
}
