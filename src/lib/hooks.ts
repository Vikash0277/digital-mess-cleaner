import { useCallback, useEffect, useRef, useState, useSyncExternalStore } from "react";
import { errorMessage } from "./api";
import { BridgeError } from "./api";

// A tiny global revision counter. Any mutation (scan finished, files
// quarantined, restored, deleted) bumps it, and every mounted data hook
// refreshes. This keeps screens honest without a query library.
let revision = 0;
const listeners = new Set<() => void>();

export function bumpData(): void {
  revision += 1;
  for (const listener of listeners) listener();
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

export function useDataRevision(): number {
  return useSyncExternalStore(
    subscribe,
    () => revision,
    () => revision,
  );
}

export interface AsyncState<T> {
  data: T | null;
  error: string | null;
  errorCode: string | null;
  loading: boolean;
  reload: () => void;
}

/**
 * Loads remote data and refreshes when the global data revision changes.
 * `loader` must be stable (pass a useCallback or a module-level function).
 */
export function useAsync<T>(loader: () => Promise<T>, deps: unknown[] = []): AsyncState<T> {
  const [data, setData] = useState<T | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [errorCode, setErrorCode] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [manualTick, setManualTick] = useState(0);
  const revisionValue = useDataRevision();
  const runId = useRef(0);

  const reload = useCallback(() => {
    setManualTick((tick) => tick + 1);
  }, []);

  useEffect(() => {
    const current = ++runId.current;
    let cancelled = false;
    setLoading(true);

    loader()
      .then((value) => {
        if (cancelled || current !== runId.current) return;
        setData(value);
        setError(null);
        setErrorCode(null);
      })
      .catch((cause: unknown) => {
        if (cancelled || current !== runId.current) return;
        setError(errorMessage(cause));
        setErrorCode(cause instanceof BridgeError ? cause.code : "unknown_error");
      })
      .finally(() => {
        if (cancelled || current !== runId.current) return;
        setLoading(false);
      });

    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [loader, revisionValue, manualTick, ...deps]);

  return { data, error, errorCode, loading, reload };
}

export function useDebouncedValue<T>(value: T, delayMs = 250): T {
  const [debounced, setDebounced] = useState(value);

  useEffect(() => {
    const timer = window.setTimeout(() => setDebounced(value), delayMs);
    return () => window.clearTimeout(timer);
  }, [value, delayMs]);

  return debounced;
}
