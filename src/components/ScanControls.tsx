import { useState } from "react";
import { Play, Square } from "lucide-react";
import { formatBytes, formatCount, pluralize } from "../lib/format";
import { useScan } from "../lib/scan";
import { ErrorNotice } from "./Notice";

export interface ScanControlsProps {
  hasRoots: boolean;
  onNeedFolder?: () => void;
}

export function ScanControls({ hasRoots, onNeedFolder }: ScanControlsProps) {
  const { running, progress, start, cancel, lastDone, error, clearError } = useScan();
  const [starting, setStarting] = useState(false);

  async function handleStart() {
    if (!hasRoots) {
      onNeedFolder?.();
      return;
    }
    setStarting(true);
    try {
      await start();
    } catch {
      // The provider already stored the message for the notice below.
    } finally {
      setStarting(false);
    }
  }

  const report = lastDone?.reports.reduce(
    (totals, item) => ({
      filesIndexed: totals.filesIndexed + item.filesIndexed,
      filesSkipped: totals.filesSkipped + item.filesSkipped,
      filesRemoved: totals.filesRemoved + item.filesRemoved,
      bytesSeen: totals.bytesSeen + item.bytesSeen,
      tookMs: totals.tookMs + item.tookMs,
    }),
    { filesIndexed: 0, filesSkipped: 0, filesRemoved: 0, bytesSeen: 0, tookMs: 0 },
  );

  return (
    <section className="scan-controls card" aria-label="Scanning">
      <div className="scan-copy">
        <p className="scan-title">{running ? "Scanning your folders…" : "Scan your folders"}</p>
        <p className="scan-meta">
          {running && progress
            ? `${formatCount(progress.filesSeen)} seen · ${formatCount(progress.filesIndexed)} indexed · ${formatBytes(progress.bytesSeen)}`
            : report
              ? `${formatCount(report.filesIndexed)} ${pluralize(report.filesIndexed, "file")} indexed in ${(report.tookMs / 1000).toFixed(1)}s${
                  report.filesRemoved > 0
                    ? ` · ${formatCount(report.filesRemoved)} removed from the index`
                    : ""
                }`
              : "Builds or refreshes the local index. Nothing leaves this device."}
        </p>
      </div>

      {running ? (
        <button type="button" className="btn" onClick={() => void cancel()}>
          <Square size={14} aria-hidden="true" />
          Cancel scan
        </button>
      ) : (
        <button
          type="button"
          className="btn btn-primary"
          onClick={() => void handleStart()}
          disabled={starting}
        >
          <Play size={14} aria-hidden="true" />
          {starting ? "Starting…" : "Scan now"}
        </button>
      )}

      {running && (
        <>
          <div
            className="progress-indeterminate"
            role="progressbar"
            aria-label="Scan in progress"
          />
          {progress?.currentPath && (
            <p className="scan-current" title={progress.currentPath}>
              {progress.currentPath}
            </p>
          )}
        </>
      )}

      {lastDone?.errors.map((message) => (
        <p className="scan-error-line" key={message}>
          {message}
        </p>
      ))}

      {error && <ErrorNotice message={error} onRetry={clearError} />}
    </section>
  );
}
