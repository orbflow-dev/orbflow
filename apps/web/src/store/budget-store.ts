import { api } from "@/lib/api";
import { setBudgetApiClient } from "@orbflow/core/stores";

// Initialize the orbflow-core budget store with our app-specific API client
setBudgetApiClient(api);

export { useBudgetStore } from "@orbflow/core/stores";
