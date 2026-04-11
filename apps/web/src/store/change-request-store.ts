import { api } from "@/lib/api";
import { setChangeRequestApiClient } from "@orbflow/core/stores";

// Initialize the orbflow-core change request store with our app-specific API client
setChangeRequestApiClient(api);

export { useChangeRequestStore } from "@orbflow/core/stores";
