import { api } from "@/lib/api";
import { setWorkflowApiClient } from "@orbflow/core/stores";

// Initialize the orbflow-core workflow store with our app-specific API client
setWorkflowApiClient(api);

export { useWorkflowStore } from "@orbflow/core/stores";
