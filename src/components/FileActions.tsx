import { ExternalLink, Eye, FolderOpen, Info } from "lucide-react";
import { useState } from "react";
import { api, errorMessage } from "../lib/api";
import type { FileEntry } from "../lib/types";
import { useFileInspector } from "./FileInspector";

export interface FileActionsProps {
  file: FileEntry;
  className?: string;
}

/** Read-only actions shared by every list of files in the app. */
export function FileActions({ file, className }: FileActionsProps) {
  const inspector = useFileInspector();
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function run(action: "open" | "reveal") {
    setBusy(true);
    setError(null);
    try {
      if (action === "open") await api.openFile(file.id);
      else await api.revealFile(file.id);
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setBusy(false);
    }
  }

  return (
    <span
      className={className ? `file-actions ${className}` : "file-actions"}
      role="group"
      aria-label={`Actions for ${file.name}`}
    >
      <button
        type="button"
        className="file-action-btn"
        onClick={() => void run("open")}
        disabled={busy}
        aria-label={`Open ${file.name} with the default app`}
        title="Open with default app"
      >
        <ExternalLink size={14} aria-hidden="true" />
      </button>
      <button
        type="button"
        className="file-action-btn"
        onClick={() => void run("reveal")}
        disabled={busy}
        aria-label={`Reveal ${file.name} in File Explorer`}
        title="Reveal in File Explorer"
      >
        <FolderOpen size={14} aria-hidden="true" />
      </button>
      <button
        type="button"
        className="file-action-btn"
        onClick={() => inspector.openPreview(file)}
        aria-label={`Preview ${file.name} inside Digital Mess Cleaner`}
        title="Preview inside the app"
      >
        <Eye size={14} aria-hidden="true" />
      </button>
      <button
        type="button"
        className="file-action-btn"
        onClick={() => inspector.openDetails(file)}
        aria-label={`Show details for ${file.name}`}
        title="Show details"
      >
        <Info size={14} aria-hidden="true" />
      </button>
      {error && (
        <span className="file-action-error" role="alert" title={error}>
          {error}
        </span>
      )}
    </span>
  );
}
