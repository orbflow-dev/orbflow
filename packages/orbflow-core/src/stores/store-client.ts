import type { ApiClient } from "../client/api-client";

/**
 * Creates a set/require pair for API client injection in Zustand stores.
 *
 * Every store that talks to the backend needs an ApiClient reference.
 * Instead of duplicating the _client / setClient / requireClient boilerplate,
 * call this factory once per store module.
 *
 * Usage:
 *   const { setClient, requireClient } = createStoreClient("Alert");
 *   export { setClient as setAlertApiClient };
 */
export function createStoreClient(storeName: string): {
  setClient: (client: ApiClient) => void;
  requireClient: () => ApiClient;
} {
  let _client: ApiClient | null = null;

  return {
    setClient(client: ApiClient) {
      _client = client;
    },
    requireClient(): ApiClient {
      if (!_client)
        throw new Error(
          `${storeName} store: API client not initialized. Call set${storeName}ApiClient() at app startup.`
        );
      return _client;
    },
  };
}
