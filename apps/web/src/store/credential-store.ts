import { api } from "@/lib/api";
import { setCredentialApiClient } from "@orbflow/core/stores";

// Initialize the orbflow-core credential store with our app-specific API client
setCredentialApiClient(api);

export { useCredentialStore } from "@orbflow/core/stores";
