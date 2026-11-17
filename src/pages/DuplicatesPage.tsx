import { Repeat, ShieldCheck } from "lucide-react";
import { useMemo, useState } from "react";
import { EmptyState } from "../components/EmptyState";
import { ErrorNotice, LoadingNotice, SuccessNotice } from "../components/Notice";
import { ProgressBar } from "../components/ProgressBar";
import { api, errorMessage } from "../lib/api";
import { formatBytes, formatCount, formatDate, pluralize } from "../lib/format";
import { bumpData } from "../lib/hooks";
import { useReview } from "../lib/review";
import type { DuplicateReport } from "../lib/types";

type Mode = "exact" | "similar";

const MODE_COPY: Record<Mode, { label: string; description: string; action: string }> = {
  exact: {
    label: "Exact duplicates",
    description:
      "Byte-identical files only. Grouped by size, then a partial hash, then a full BLAKE3 hash.",
    action: "Find exact duplicates",
  },
  similar: {
    label: "Similar images",
    description:
      "Visually similar photos found with a perceptual hash. These are never treated as exact copies.",
    action: "Find similar images",
  },
};

export function DuplicatesPage() {
  const review = useReview();
  const [mode, setMode] = useState<Mode>("exact");
  const [threshold, setThreshold] = useState(10);
  const [report, setReport] = useState<DuplicateReport | null>(null);
  const [analyzing, setAnalyzing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [selected, setSelected] = useState<Set<number>>(new Set());

  const selectedFiles = useMemo(() => {
    if (!report) return [];
    return report.groups.flatMap((group) => group.files).filter((file) => selected.has(file.id));
  }, [report, selected]);

  const selectedBytes = selectedFiles.reduce((sum, file) => sum + file.size, 0);

  async function analyze(nextMode: Mode = mode) {
    setAnalyzing(true);
    setError(null);
    setNotice(null);
    setSelected(new Set());
    try {
      const result = await api.duplicatesAnalyze(nextMode, threshold);
      setReport(result);
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setAnalyzing(false);
    }
  }

  function switchMode(next: Mode) {
    setMode(next);
    setReport(null);
    setSelected(new Set());
    setError(null);
    setNotice(null);
  }

  function toggle(id: number) {
    setSelected((current) => {
      const next = new Set(current);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }

  function selectDuplicatesInGroup(groupId: string) {
    if (!report) return;
    const group = report.groups.find((item) => item.id === groupId);
    if (!group) return;

    setSelected((current) => {
      const next = new Set(current);
      const keepNewest = [...group.files].sort((a, b) => b.mtime - a.mtime)[0];
      for (const file of group.files) {
        if (file.id !== keepNewest?.id) next.add(file.id);
      }
      return next;
    });
  }

  async function quarantineSelected() {
    const ids = selectedFiles.map((file) => file.id);
    if (ids.length === 0) return;

    const confirmed = await review({
      title: `Move ${ids.length} ${pluralize(ids.length, "file")} to quarantine?`,
      message:
        "The selected copies move to the app-managed quarantine. Keep at least one copy of anything you still need.",
      details: [
        { label: "Files", value: formatCount(ids.length) },
        { label: "Total size", value: formatBytes(selectedBytes) },
        { label: "Duplicate type", value: mode === "exact" ? "Byte-identical" : "Visually similar" },
      ],
      note:
        mode === "exact"
          ? "Reversible. Restore from the Cleanup Centre at any time."
          : "Similar images are not guaranteed to be identical. Check them before removing.",
      primaryText: "Move to quarantine",
      secondaryText: "Keep files",
      tone: mode === "exact" ? "caution" : "danger",
    });
    if (!confirmed) return;

    setError(null);
    setNotice(null);
    try {
      const result = await api.quarantineFiles(ids);
      bumpData();
      setNotice(
        `Moved ${formatCount(result.moved)} ${pluralize(result.moved, "file")} to quarantine (${formatBytes(result.totalBytes)}).${
          result.failed > 0 ? ` ${result.failed} could not be moved.` : ""
        }`,
      );
      await analyze();
    } catch (cause) {
      setError(errorMessage(cause));
    }
  }

  const copy = MODE_COPY[mode];
  const totalWasted = report?.totalWasted ?? 0;

  return (
    <div className="page">
      <div className="page-heading">
        <div>
          <h1 className="page-title">Duplicate Cleaner</h1>
          <p className="page-subtitle">Find and remove duplicate files safely</p>
        </div>
      </div>

      <div className="segmented" role="tablist" aria-label="Duplicate type">
        {(Object.keys(MODE_COPY) as Mode[]).map((option) => (
          <button
            key={option}
            type="button"
            role="tab"
            aria-selected={mode === option}
            className="segment"
            data-active={mode === option || undefined}
            onClick={() => switchMode(option)}
          >
            {MODE_COPY[option].label}
          </button>
        ))}
      </div>

      <section className="card duplicates-intro" aria-label="How duplicates are detected">
        <p>{copy.description}</p>
        {mode === "similar" && (
          <label className="filter-field">
            <span>Match strictness</span>
            <select
              value={threshold}
              onChange={(event) => setThreshold(Number(event.target.value))}
            >
              <option value={6}>Strict — closest matches only</option>
              <option value={10}>Balanced</option>
              <option value={16}>Loose — more suggestions</option>
            </select>
          </label>
        )}
        <button
          type="button"
          className="btn btn-primary"
          onClick={() => void analyze()}
          disabled={analyzing}
        >
          <Repeat size={14} aria-hidden="true" />
          {analyzing ? "Comparing files…" : copy.action}
        </button>
      </section>

      {analyzing && <LoadingNotice label="Hashing candidates and grouping matches…" />}
      {error && <ErrorNotice message={error} onRetry={() => void analyze()} />}
      {notice && <SuccessNotice message={notice} />}

      {report && !analyzing && (
        <section className="card duplicates-summary" aria-label="Duplicate summary">
          <div className="duplicates-summary-stats">
            <div>
              <span className="stat-label">Groups</span>
              <span className="stat-number">{formatCount(report.groups.length)}</span>
            </div>
            <div>
              <span className="stat-label">Could be reclaimed</span>
              <span className="stat-number">{formatBytes(totalWasted)}</span>
            </div>
            <div>
              <span className="stat-label">Candidates checked</span>
              <span className="stat-number">{formatCount(report.candidateFiles)}</span>
            </div>
            <div>
              <span className="stat-label">Fully hashed</span>
              <span className="stat-number">{formatCount(report.hashedFiles)}</span>
            </div>
          </div>
          <ProgressBar
            progress={
              report.candidateFiles > 0 ? (report.hashedFiles / report.candidateFiles) * 100 : 100
            }
            label={`${formatCount(report.hashedFiles)} of ${formatCount(report.candidateFiles)} hashed`}
          />
          {report.notes.length > 0 && (
            <ul className="notes-list">
              {report.notes.map((note) => (
                <li key={note}>{note}</li>
              ))}
            </ul>
          )}
        </section>
      )}

      {selectedFiles.length > 0 && (
        <div className="selection-bar" role="region" aria-label="Selected duplicates">
          <span>
            {formatCount(selectedFiles.length)} {pluralize(selectedFiles.length, "file")} selected ·{" "}
            {formatBytes(selectedBytes)}
          </span>
          <div className="selection-actions">
            <button
              type="button"
              className="btn btn-caution"
              onClick={() => void quarantineSelected()}
            >
              <ShieldCheck size={14} aria-hidden="true" />
              Move to quarantine
            </button>
            <button type="button" className="btn" onClick={() => setSelected(new Set())}>
              Clear selection
            </button>
          </div>
        </div>
      )}

      {!report && !analyzing && !error && (
        <EmptyState
          title={mode === "exact" ? "No duplicate scan yet" : "No similar-image scan yet"}
          description={
            mode === "exact"
              ? "Run the check to group byte-identical files. Nothing is removed automatically."
              : "Run the check to find photos that look alike. Similar never means identical."
          }
          icon="repeat"
          buttonText={copy.action}
          onAction={() => void analyze()}
        />
      )}

      {report && report.groups.length === 0 && (
        <EmptyState
          title="No duplicates found"
          description={
            mode === "exact"
              ? "Every indexed file has unique content."
              : "No images passed this strictness. Try a looser setting."
          }
          icon="repeat"
        />
      )}

      {report && report.groups.length > 0 && (
        <div className="duplicate-groups">
          {report.groups.map((group) => (
            <section
              key={group.id}
              className="duplicate-group"
              aria-labelledby={`group-${group.id}`}
            >
              <div className="group-header">
                <span id={`group-${group.id}`}>
                  {group.kind === "exact" ? "Exact duplicates" : "Similar images"} (
                  {group.files.length})
                </span>
                <span>
                  {group.kind === "exact"
                    ? `Save ${formatBytes(group.wastedBytes)}`
                    : `${Math.round((group.similarity ?? 0) * 100)}% similar · ${formatBytes(group.wastedBytes)}`}
                </span>
              </div>

              <div className="group-files">
                {group.files.map((file) => {
                  const isSelected = selected.has(file.id);
                  return (
                    <label
                      key={file.id}
                      className="group-file-row"
                      data-selected={isSelected || undefined}
                    >
                      <input
                        type="checkbox"
                        checked={isSelected}
                        onChange={() => toggle(file.id)}
                        aria-label={`Select ${file.name}`}
                      />
                      <span className="group-file-name" title={file.path}>
                        {file.name}
                      </span>
                      <span className="group-file-path" title={file.parentPath}>
                        {file.parentPath}
                      </span>
                      <span className="group-file-size">{formatBytes(file.size)}</span>
                      <span className="group-file-date">{formatDate(file.mtime)}</span>
                    </label>
                  );
                })}
              </div>

              <div className="group-actions">
                {group.kind !== "exact" && (
                  <span className="badge badge-caution">Similar, not byte-identical</span>
                )}
                <button
                  type="button"
                  className="btn"
                  onClick={() => selectDuplicatesInGroup(group.id)}
                >
                  Select all but the newest
                </button>
              </div>
            </section>
          ))}
        </div>
      )}
    </div>
  );
}
