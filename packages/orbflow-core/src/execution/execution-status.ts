export type ExecutionStatus =
  | "pending"
  | "queued"
  | "running"
  | "completed"
  | "failed"
  | "skipped"
  | "cancelled"
  | "waiting_approval";
