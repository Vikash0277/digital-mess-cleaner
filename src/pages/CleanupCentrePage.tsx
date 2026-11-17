import { History, RotateCcw, ShieldCheck, Trash2 } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { EmptyState } from "../components/EmptyState";
import { ErrorNotice, LoadingNotice, SuccessNotice } from "../components/Notice";
import { api, errorMessage } from "../lib/api";
import {
  formatBytes,
  formatCount,
  formatDateTime,
  pluralize,
  relativeTime,
} from "../lib/format";
import { bumpData, useAsync } from "../lib/hooks";
import { useReview } from "../lib/review";
import type { OperationEntry, QuarantineEntry } from "../lib/types";

type Tab = "quarantine" | "history";

const loadCleanup = (includeRestored: boolean) =>
  Promise.all([api.quarantineList(includeRestored), api.operationsList(50)]).then(
    ([entries, operations]) => ({ entries, operations }),
  );

function stateLabel(entry: QuarantineEntry): string {
  switch (entry.state) {
    case "held":
      return "Held in quarantine";
    case "restored":
      return "Restored";
    case "deleted":
      return "Permanently deleted";
    default:
      return entry.state;
  }
}

export function CleanupCentrePage() {
  const review = useReview();
  const [tab, setTab] = useState<Tab>("quarantine");
  const [includeRestored, setIncludeRestored] = useState(false);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [busy, setBusy] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const loader = useCallback(() => loadCleanup(includeRestored), [includeRestored]);
  const cleanup = useAsync(loader);

  useEffect(() => {
    setSelected(new Set());
  }, [includeRestored]);

  const entries = useMemo(() => cleanup.data?.entries ?? [], [cleanup.data]);
  const operations = useMemo(() => cleanup.data?.operations ?? [], [cleanup.data]);
  const held = useMemo(() => entries.filter((entry) => entry.state === "held"), [entries]);
  const selectedEntries = useMemo(
    () => held.filter((entry) => selected.has(entry.id)),
    [held, selected],
  );
  const selectedBytes = selectedEntries.reduce((sum, entry) => sum + entry.size, 0);
  const heldBytes = held.reduce((sum, entry) => sum + entry.size, 0);

  function toggle(id: string) {
    setSelected((current) => {
      const next = new Set(current);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }

  async function restoreSelected() {
    const ids = selectedEntries.map((entry) => entry.id);
    if (ids.length === 0) return;

    const confirmed = await review({
      title: `Restore ${ids.length} ${pluralize(ids.length, "file")}?`,
      message:
        "Each file goes back to the folder it came from. If a file with the same name is there now, the restore stops for that file instead of overwriting it.",
      details: [
        { label: "Files", value: formatCount(ids.length) },
        { label: "Total size", value: formatBytes(selectedBytes) },
        { label: "Destination", value: "Their original folders" },
      ],
      note: "Reversible by quarantining them again. No file on disk is overwritten.",
      primaryText: "Restore files",
      secondaryText: "Keep in quarantine",
      tone: "caution",
    });
    if (!confirmed) return;

    setActionError(null);
    setNotice(null);
    setBusy(true);
    try {
      const result = await api.quarantineRestore(ids);
      bumpData();
      setSelected(new Set());
      setNotice(
        `Restored ${formatCount(result.moved)} ${pluralize(result.moved, "file")}.${
          result.failed > 0 ? ` ${result.failed} could not be restored — they stay in quarantine.` : ""
        }`,
      );
    } catch (error) {
      setActionError(errorMessage(error));
    } finally {
      setBusy(false);
    }
  }

  async function deleteSelectedPermanently() {
    const ids = selectedEntries.map((entry) => entry.id);
    if (ids.length === 0) return;

    const confirmed = await review({
      title: `Permanently delete ${ids.length} ${pluralize(ids.length, "file")}?`,
      message:
        "This erases the quarantined copies from disk. The originals were already moved here, so they cannot be recovered afterwards.",
      details: [
        { label: "Files", value: formatCount(ids.length) },
        { label: "Total size", value: formatBytes(selectedBytes) },
        { label: "Where", value: "Digital Mess Cleaner quarantine" },
      ],
      note: "This cannot be undone. Restore the files instead if you are unsure.",
      confirmPhrase: "DELETE PERMANENTLY",
      primaryText: "Delete permanently",
      secondaryText: "Cancel",
      tone: "danger",
    });
    if (!confirmed) return;

    setActionError(null);
    setNotice(null);
    setBusy(true);
    try {
      const result = await api.quarantinePermanentDelete(ids, "DELETE PERMANENTLY");
      bumpData();
      setSelected(new Set());
      setNotice(
        `Permanently deleted ${formatCount(result.moved)} ${pluralize(result.moved, "file")} (${formatBytes(result.totalBytes)}).`,
      );
    } catch (error) {
      setActionError(errorMessage(error));
    } finally {
      setBusy(false);
    }
  }

  async function undoOperation(entry: OperationEntry) {
    const confirmed = await review({
      title: "Undo this operation?",
      message: entry.summary,
      details: [
        { label: "When", value: formatDateTime(entry.createdAt) },
        { label: "Items", value: formatCount(entry.itemCount) },
        { label: "Size", value: formatBytes(entry.bytes) },
      ],
      note: entry.reversible
        ? "Files are put back where they were. If a name clash appeared since then, that item is skipped rather than overwritten."
        : "This operation was recorded as not reversible.",
      primaryText: "Undo operation",
      secondaryText: "Leave as is",
      tone: "caution",
    });
    if (!confirmed) return;

    setActionError(null);
    setNotice(null);
    setBusy(true);
    try {
      await api.operationUndo(entry.id);
      bumpData();
      setNotice(`Undid: ${entry.summary}`);
    } catch (error) {
      setActionError(errorMessage(error));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="page">
      <div className="page-heading">
        <div>
          <h1 className="page-title">Cleanup Centre</h1>
          <p className="page-subtitle">
            Everything removed waits here first. Restore it or delete it deliberately.
          </p>
        </div>
      </div>

      <div className="segmented" role="tablist" aria-label="Cleanup view">
        <button
          type="button"
          role="tab"
          aria-selected={tab === "quarantine"}
          className="segment"
          data-active={tab === "quarantine" || undefined}
          onClick={() => setTab("quarantine")}
        >
          Quarantine ({formatCount(held.length)})
        </button>
        <button
          type="button"
          role="tab"
          aria-selected={tab === "history"}
          className="segment"
          data-active={tab === "history" || undefined}
          onClick={() => setTab("history")}
        >
          History ({formatCount(operations.length)})
        </button>
      </div>

      {actionError && <ErrorNotice message={actionError} />}
      {notice && <SuccessNotice message={notice} />}

      {cleanup.loading && !cleanup.data && (
        <LoadingNotice label="Reading the quarantine and operation log…" />
      )}

      {cleanup.error && (
        <ErrorNotice
          message={cleanup.error}
          onRetry={cleanup.reload}
          desktopOnly={cleanup.errorCode === "not_desktop"}
        />
      )}

      {tab === "quarantine" && cleanup.data && (
        <>
          <div className="card quarantine-summary">
            <div className="section-head">
              <div>
                <h2 className="section-title">Held safely</h2>
                <p className="muted">
                  {formatCount(held.length)} {pluralize(held.length, "file")} ·{" "}
                  {formatBytes(heldBytes)}. Nothing here has been permanently deleted.
                </p>
              </div>
              <label className="filter-field">
                <span>Show restored items</span>
                <input
                  type="checkbox"
                  checked={includeRestored}
                  onChange={(event) => setIncludeRestored(event.target.checked)}
                />
              </label>
            </div>

            {held.length === 0 ? (
              <EmptyState
                title="Quarantine is empty"
                description="When you move files here from My Files, Duplicates or Forgotten Files, they appear in this list with a way back."
                icon="trash"
              />
            ) : (
              <>
                {selectedEntries.length > 0 && (
                  <div className="selection-bar" role="region" aria-label="Selected quarantined files">
                    <span>
                      {formatCount(selectedEntries.length)}{" "}
                      {pluralize(selectedEntries.length, "file")} selected ·{" "}
                      {formatBytes(selectedBytes)}
                    </span>
                    <div className="selection-actions">
                      <button
                        type="button"
                        className="btn btn-primary"
                        onClick={() => void restoreSelected()}
                        disabled={busy}
                      >
                        <RotateCcw size={14} aria-hidden="true" />
                        Restore to original folder
                      </button>
                      <button
                        type="button"
                        className="btn btn-danger"
                        onClick={() => void deleteSelectedPermanently()}
                        disabled={busy}
                      >
                        <Trash2 size={14} aria-hidden="true" />
                        Delete permanently
                      </button>
                      <button
                        type="button"
                        className="btn"
                        onClick={() => setSelected(new Set())}
                      >
                        Clear selection
                      </button>
                    </div>
                  </div>
                )}

                <ul className="quarantine-list">
                  {entries.map((entry) => {
                    const isHeld = entry.state === "held";
                    return (
                      <li key={entry.id} className="quarantine-item">
                        {isHeld ? (
                          <input
                            type="checkbox"
                            checked={selected.has(entry.id)}
                            onChange={() => toggle(entry.id)}
                            aria-label={`Select ${entry.name}`}
                          />
                        ) : (
                          <span className="quarantine-check" aria-hidden="true" />
                        )}
                        <div className="quarantine-info">
                          <span className="quarantine-name" title={entry.originalPath}>
                            {entry.name}
                          </span>
                          <span className="quarantine-path" title={entry.originalPath}>
                            {entry.originalPath}
                          </span>
                        </div>
                        <span className="quarantine-size">{formatBytes(entry.size)}</span>
                        <span className="badge" data-state={entry.state}>
                          {stateLabel(entry)}
                        </span>
                        <span className="quarantine-meta">
                          {isHeld
                            ? `Held ${entry.daysHeld} ${pluralize(entry.daysHeld, "day")} · since ${relativeTime(entry.createdAt)}`
                            : entry.restoredAt
                              ? `${stateLabel(entry)} ${relativeTime(entry.restoredAt)}`
                              : stateLabel(entry)}
                        </span>
                        {isHeld && !entry.fileExists && (
                          <span className="badge badge-warning">
                            Copy missing on disk — restore may fail
                          </span>
                        )}
                      </li>
                    );
                  })}
                </ul>
              </>
            )}
          </div>

          <section className="card" aria-label="How quarantine works">
            <h2 className="section-title">
              <ShieldCheck size={16} aria-hidden="true" /> How this keeps you safe
            </h2>
            <ul className="notes-list">
              <li>Quarantine moves files; it never deletes them.</li>
              <li>Restore puts each file back at its original path.</li>
              <li>Permanent delete is separate and needs the exact phrase.</li>
              <li>Every action is written to the history list on the next tab.</li>
            </ul>
          </section>
        </>
      )}

      {tab === "history" && cleanup.data && (
        <section className="card" aria-labelledby="history-title">
          <div className="section-head">
            <h2 id="history-title" className="section-title">
              <History size={16} aria-hidden="true" /> Operation history
            </h2>
            <span className="muted">Newest first · {formatCount(operations.length)} entries</span>
          </div>

          {operations.length === 0 ? (
            <EmptyState
              title="No operations yet"
              description="Quarantine, restore and delete actions are logged here as soon as they happen."
              icon="history"
            />
          ) : (
            <ul className="activity-list">
              {operations.map((entry) => (
                <li key={entry.id} className="activity-item">
                  <span className="activity-summary">{entry.summary}</span>
                  <span className="activity-meta">
                    {formatDateTime(entry.createdAt)} · {formatCount(entry.itemCount)}{" "}
                    {pluralize(entry.itemCount, "item")} · {formatBytes(entry.bytes)} ·{" "}
                    {entry.undoneAt
                      ? `Undone ${relativeTime(entry.undoneAt)}`
                      : entry.reversible
                        ? "Reversible"
                        : "Not reversible"}
                  </span>
                  {entry.reversible && !entry.undoneAt && (
                    <button
                      type="button"
                      className="btn"
                      onClick={() => void undoOperation(entry)}
                      disabled={busy}
                    >
                      <RotateCcw size={14} aria-hidden="true" />
                      Undo
                    </button>
                  )}
                </li>
              ))}
            </ul>
          )}
        </section>
      )}
    </div>
  );
}
