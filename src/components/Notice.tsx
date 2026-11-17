import { AlertCircle, CheckCircle2, SearchX } from "lucide-react";
import type { ReactNode } from "react";

export function ErrorNotice({
  message,
  onRetry,
  desktopOnly = false,
}: {
  message: string;
  onRetry?: () => void;
  desktopOnly?: boolean;
}) {
  return (
    <div className="notice notice-error" role="alert">
      <AlertCircle size={16} aria-hidden="true" />
      <div className="notice-body">
        <p>{message}</p>
        {desktopOnly && (
          <p className="notice-hint">
            Open the project with <code>npm run tauri dev</code> to connect to your files.
          </p>
        )}
      </div>
      {onRetry && (
        <button type="button" className="btn" onClick={onRetry}>
          Try again
        </button>
      )}
    </div>
  );
}

export function SuccessNotice({
  message,
  children,
}: {
  message: string;
  children?: ReactNode;
}) {
  return (
    <div className="notice notice-success" role="status">
      <CheckCircle2 size={16} aria-hidden="true" />
      <div className="notice-body">
        <p>{message}</p>
        {children}
      </div>
    </div>
  );
}

export function LoadingNotice({ label = "Loading…" }: { label?: string }) {
  return (
    <div className="notice notice-muted" role="status">
      <span className="spinner" aria-hidden="true" />
      <p>{label}</p>
    </div>
  );
}

export function NoResults({ message }: { message: string }) {
  return (
    <div className="notice notice-muted">
      <SearchX size={16} aria-hidden="true" />
      <p>{message}</p>
    </div>
  );
}
