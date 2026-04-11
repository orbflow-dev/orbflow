import { api } from "@/lib/api";
import { setAlertApiClient } from "@orbflow/core/stores";

// Initialize the orbflow-core alert store with our app-specific API client
setAlertApiClient(api);

export { useAlertStore } from "@orbflow/core/stores";
