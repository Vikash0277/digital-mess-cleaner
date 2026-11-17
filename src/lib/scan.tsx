import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { api, isDesktop } from "./api";
import { bumpData } from "./hooks";
import type { ScanDone, ScanProgress } from "./types";

interface ScanContextValue {
  running: boolean;
  scanId: string | null;
  progress: ScanProgress | null;
  lastDone: ScanDone | null;
  error: string | null;
  start: (rootIds?: number[]) => Promise<void>;
  cancel: () => Promise<void>;
  clearError: () => void;
}

const ScanContext = createContext<ScanContextValue>({
  running: false,
  scanId: null,
  progress: null,
  lastDone: null,
  error: null,
  start: async () => {},
  cancel: async () => {},
  clearError: () => {},
});

interface ScanEventPayload {
  scanId?: string;
  reports?: ScanDone["reports"];
  errors?: string[];
  cancelled?: boolean;
}

export function ScanProvider({ children }: { children: ReactNode }) {
  const [running, setRunning] = useState(false);
  const [scanId, setScanId] = useState<string | null>(null);
  const [progress, setProgress] = useState<ScanProgress | null>(null);
  const [lastDone, setLastDone] = useState<ScanDone | null>(null);
  const [error, setError] = useState<string | null>(null);
  const activeId = useRef<string | null>(null);

  useEffect(() => {
    if (!isDesktop()) return;

    let disposed = false;
    const cleanups: (() => void)[] = [];

    void (async () => {
      const { listen } = await import("@tauri-apps/api/event");

      const unlistenProgress = await listen<ScanProgress>("scan://progress", (event) => {
        if (disposed) return;
        if (activeId.current && event.payload.scanId !== activeId.current) return;
        setProgress(event.payload);
      });
      const unlistenDone = await listen<ScanEventPayload>("scan://done", (event) => {
        if (disposed) return;
        if (activeId.current && event.payload.scanId !== activeId.current) return;
        activeId.current = null;
        setRunning(false);
        setScanId(null);
        const payload = event.payload;
        setLastDone({
          scanId: payload.scanId ?? "",
          reports: payload.reports ?? [],
          errors: payload.errors ?? [],
          cancelled: payload.cancelled ?? false,
        });
        if (payload.errors && payload.errors.length > 0) {
          setError(payload.errors[0] ?? "The scan finished with errors.");
        }
        setProgress(null);
        bumpData();
      });

      cleanups.push(unlistenProgress, unlistenDone);
    })();

    return () => {
      disposed = true;
      for (const cleanup of cleanups) cleanup();
    };
  }, []);

  const start = useCallback(async (rootIds?: number[]) => {
    setError(null);
    setLastDone(null);
    setProgress(null);
    try {
      const id = await api.scanStart(rootIds);
      activeId.current = id;
      setScanId(id);
      setRunning(true);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
      throw cause;
    }
  }, []);

  const cancel = useCallback(async () => {
    const id = activeId.current;
    if (!id) return;
    try {
      await api.scanCancel(id);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  }, []);

  const clearError = useCallback(() => setError(null), []);

  return (
    <ScanContext.Provider
      value={{ running, scanId, progress, lastDone, error, start, cancel, clearError }}
    >
      {children}
    </ScanContext.Provider>
  );
}

export function useScan(): ScanContextValue {
  return useContext(ScanContext);
}
