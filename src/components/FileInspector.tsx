import {
  ExternalLink,
  Eye,
  FileText,
  FolderOpen,
  Info,
  X,
} from "lucide-react";
import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useId,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { api, errorMessage } from "../lib/api";
import { categoryLabel, formatBytes, formatDate, formatDateTime } from "../lib/format";
import type { FileDetails, FileEntry, FilePreview } from "../lib/types";

type InspectorMode = "details" | "preview";

interface FileInspectorContextValue {
  openDetails: (file: FileEntry) => void;
  openPreview: (file: FileEntry) => void;
}

const noop = () => {};
const FileInspectorContext = createContext<FileInspectorContextValue>({
  openDetails: noop,
  openPreview: noop,
});

export function useFileInspector(): FileInspectorContextValue {
  return useContext(FileInspectorContext);
}

export function FileInspectorProvider({ children }: { children: ReactNode }) {
  const [file, setFile] = useState<FileEntry | null>(null);
  const [mode, setMode] = useState<InspectorMode>("details");

  const openDetails = useCallback((next: FileEntry) => {
    setFile(next);
    setMode("details");
  }, []);
  const openPreview = useCallback((next: FileEntry) => {
    setFile(next);
    setMode("preview");
  }, []);
  const close = useCallback(() => setFile(null), []);

  return (
    <FileInspectorContext.Provider value={{ openDetails, openPreview }}>
      {children}
      <FileInspector file={file} mode={mode} onModeChange={setMode} onClose={close} />
    </FileInspectorContext.Provider>
  );
}

interface FileInspectorProps {
  file: FileEntry | null;
  mode: InspectorMode;
  onModeChange: (mode: InspectorMode) => void;
  onClose: () => void;
}

function FileInspector({ file, mode, onModeChange, onClose }: FileInspectorProps) {
  const dialogRef = useRef<HTMLDivElement>(null);
  const titleId = useId();
  const [details, setDetails] = useState<FileDetails | null>(null);
  const [preview, setPreview] = useState<FilePreview | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [blobUrl, setBlobUrl] = useState<string | null>(null);

  useEffect(() => {
    if (!file) {
      setDetails(null);
      setPreview(null);
      setBlobUrl(null);
      return;
    }

    let cancelled = false;
    setLoading(true);
    setError(null);
    setActionError(null);
    setDetails(null);
    setPreview(null);

    const request =
      mode === "details" ? api.fileDetails(file.id) : api.filePreview(file.id);
    request
      .then((result) => {
        if (cancelled) return;
        if (mode === "details") setDetails(result as FileDetails);
        else setPreview(result as FilePreview);
      })
      .catch((cause: unknown) => {
        if (!cancelled) setError(errorMessage(cause));
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });

    return () => {
      cancelled = true;
    };
  }, [file, mode]);

  useEffect(() => {
    if (!preview || (preview.kind !== "image" && preview.kind !== "pdf") || !preview.data) {
      setBlobUrl(null);
      return;
    }
    const binary = atob(preview.data);
    const bytes = new Uint8Array(binary.length);
    for (let index = 0; index < binary.length; index += 1) {
      bytes[index] = binary.charCodeAt(index);
    }
    const type = preview.mime || (preview.kind === "pdf" ? "application/pdf" : "application/octet-stream");
    const url = URL.createObjectURL(new Blob([bytes], { type }));
    setBlobUrl(url);
    return () => URL.revokeObjectURL(url);
  }, [preview]);

  useEffect(() => {
    if (!file) return;
    const previousFocus = document.activeElement as HTMLElement | null;
    dialogRef.current?.focus();

    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") {
        event.preventDefault();
        onClose();
        return;
      }
      if (event.key !== "Tab" || !dialogRef.current) return;

      const focusable = Array.from(
        dialogRef.current.querySelectorAll<HTMLElement>(
          'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), iframe, [tabindex]:not([tabindex="-1"])',
        ),
      );
      const first = focusable[0];
      const last = focusable[focusable.length - 1];
      if (!first || !last) {
        event.preventDefault();
        return;
      }
      if (event.shiftKey && document.activeElement === first) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first.focus();
      }
    }

    document.addEventListener("keydown", handleKeyDown);
    return () => {
      document.removeEventListener("keydown", handleKeyDown);
      previousFocus?.focus();
    };
  }, [file, onClose]);

  if (!file) return null;

  async function runAction(action: "open" | "reveal") {
    setBusy(true);
    setActionError(null);
    try {
      if (action === "open") await api.openFile(file!.id);
      else await api.revealFile(file!.id);
    } catch (cause) {
      setActionError(errorMessage(cause));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="dialog-backdrop">
      <div
        ref={dialogRef}
        className="dialog file-inspector"
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        tabIndex={-1}
      >
        <div className="dialog-header">
          <div className="file-inspector-title">
            <h3 id={titleId} className="dialog-title" title={file.path}>
              {file.name}
            </h3>
            <p className="muted">{formatBytes(file.size)} · {categoryLabel(file.category)}</p>
          </div>
          <button type="button" className="btn" onClick={onClose} aria-label="Close file inspector">
            <X size={14} aria-hidden="true" />
          </button>
        </div>

        <div className="inspector-tabs" role="tablist" aria-label="File inspector views">
          <button
            type="button"
            role="tab"
            aria-selected={mode === "details"}
            className={mode === "details" ? "tab active" : "tab"}
            onClick={() => onModeChange("details")}
          >
            <Info size={14} aria-hidden="true" />
            Details
          </button>
          <button
            type="button"
            role="tab"
            aria-selected={mode === "preview"}
            className={mode === "preview" ? "tab active" : "tab"}
            onClick={() => onModeChange("preview")}
          >
            <Eye size={14} aria-hidden="true" />
            Preview
          </button>
        </div>

        <div className="dialog-body inspector-body" role="tabpanel">
          {loading && <p className="muted">Reading this file locally…</p>}
          {error && <p className="file-inspector-error" role="alert">{error}</p>}

          {mode === "details" && details && (
            <div className="inspector-details">
              <p className="muted" title={details.rootPath}>
                Chosen folder: {details.rootLabel}
              </p>
              <dl className="detail-list">
                <div>
                  <dt>Full path</dt>
                  <dd className="wrap-text">{details.file.path}</dd>
                </div>
                <div>
                  <dt>Parent folder</dt>
                  <dd className="wrap-text">{details.file.parentPath}</dd>
                </div>
                <div>
                  <dt>Indexed size</dt>
                  <dd>{formatBytes(details.file.size)}</dd>
                </div>
                <div>
                  <dt>Size on disk now</dt>
                  <dd>
                    {details.exists ? formatBytes(details.currentSize) : "File is no longer there"}
                  </dd>
                </div>
                <div>
                  <dt>Modified (indexed)</dt>
                  <dd>{formatDate(details.file.mtime)}</dd>
                </div>
                <div>
                  <dt>Modified (now)</dt>
                  <dd>{details.exists ? formatDateTime(details.currentMtime) : "Unavailable"}</dd>
                </div>
                <div>
                  <dt>Created</dt>
                  <dd>{formatDateTime(details.file.ctime)}</dd>
                </div>
                <div>
                  <dt>Last accessed</dt>
                  <dd>{formatDateTime(details.file.atime)}</dd>
                </div>
                <div>
                  <dt>Type</dt>
                  <dd>{details.file.mime || categoryLabel(details.file.category)}</dd>
                </div>
                <div>
                  <dt>Extension</dt>
                  <dd>{details.file.ext ? `.${details.file.ext}` : "None"}</dd>
                </div>
                <div>
                  <dt>Confidence</dt>
                  <dd>{Math.round(details.file.confidence * 100)}%</dd>
                </div>
                <div>
                  <dt>Content hash</dt>
                  <dd>{details.file.hash ? "Verified by full-content hash" : "Not hashed"}</dd>
                </div>
                <div>
                  <dt>Flags</dt>
                  <dd>
                    {details.file.sensitive ? "Sensitive · " : ""}
                    {details.file.analyzed ? "Analysed locally" : "Not analysed"}
                  </dd>
                </div>
              </dl>
              {details.file.tags.length > 0 && (
                <p className="muted">Tags: {details.file.tags.join(", ")}</p>
              )}
            </div>
          )}

          {mode === "preview" && preview && (
            <div className="file-preview">
              {preview.kind === "text" && preview.text !== null && (
                <pre className="file-preview-text">{preview.text}</pre>
              )}
              {preview.kind === "image" && blobUrl && (
                <img className="file-preview-image" src={blobUrl} alt={file.name} />
              )}
              {preview.kind === "pdf" && blobUrl && (
                <iframe className="file-preview-pdf" src={blobUrl} title={`Preview of ${file.name}`} />
              )}
              {(preview.kind === "unsupported" || preview.kind === "too_large") && (
                <div className="file-preview-fallback">
                  <FileText size={24} aria-hidden="true" />
                  <p>{preview.reason}</p>
                </div>
              )}
              {preview.kind === "image" && !blobUrl && !error && (
                <p className="muted">Preparing image preview…</p>
              )}
              {preview.kind === "pdf" && !blobUrl && !error && (
                <p className="muted">Preparing PDF preview…</p>
              )}
            </div>
          )}
        </div>

        <div className="dialog-footer inspector-footer">
          {actionError && (
            <p className="file-inspector-error" role="alert">{actionError}</p>
          )}
          <button
            type="button"
            className="btn"
            onClick={() => void runAction("reveal")}
            disabled={busy}
          >
            <FolderOpen size={14} aria-hidden="true" />
            Reveal in File Explorer
          </button>
          <button
            type="button"
            className="btn btn-primary"
            onClick={() => void runAction("open")}
            disabled={busy}
          >
            <ExternalLink size={14} aria-hidden="true" />
            Open with default app
          </button>
          <button type="button" className="btn" onClick={onClose}>
            Close
          </button>
        </div>
      </div>
    </div>
  );
}
