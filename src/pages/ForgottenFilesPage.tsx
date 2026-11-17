import { Clock, ShieldCheck } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { EmptyState } from "../components/EmptyState";
import { FileTable } from "../components/FileTable";
import { ErrorNotice, LoadingNotice, SuccessNotice } from "../components/Notice";
import { StatCard } from "../components/StatCard";
import { api, errorMessage } from "../lib/api";
import { formatBytes, formatCount, pluralize } from "../lib/format";
import { bumpData, useAsync } from "../lib/hooks";
import { useReview } from "../lib/review";

const AGE_OPTIONS = [
  { days: 180, label: "6 months" },
  { days: 365, label: "1 year" },
  { days: 730, label: "2 years" },
  { days: 1825, label: "5 years" },
];

const SIZE_OPTIONS = [
  { bytes: 0, label: "Any size" },
  { bytes: 10 * 1024 * 1024, label: "Over 10 MB" },
  { bytes: 100 * 1024 * 1024, label: "Over 100 MB" },
  { bytes: 1024 * 1024 * 1024, label: "Over 1 GB" },
];

const LIMIT = 300;

export function ForgottenFilesPage() {
  const review = useReview();
  const [days, setDays] = useState(365);
  const [minSize, setMinSize] = useState(0);
  const [selected, setSelected] = useState<Set<number>>(new Set());
  const [busy, setBusy] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const loader = useCallback(
    () => api.forgottenFiles(days, minSize, LIMIT),
    [days, minSize],
  );
  const forgotten = useAsync(loader);

  useEffect(() => {
    setSelected(new Set());
  }, [days, minSize]);

  const files = useMemo(() => forgotten.data ?? [], [forgotten.data]);
  const selectedFiles = useMemo(
    () => files.filter((file) => selected.has(file.id)),
    [files, selected],
  );
  const selectedBytes = selectedFiles.reduce((sum, file) => sum + file.size, 0);
  const totalBytes = files.reduce((sum, file) => sum + file.size, 0);

  function toggle(id: number) {
    setSelected((current) => {
      const next = new Set(current);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }

  function toggleAll(checked: boolean) {
    setSelected((current) => {
      const next = new Set(current);
      for (const file of files) {
        if (checked) next.add(file.id);
        else next.delete(file.id);
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
        "Old files can still matter — tax records, certificates, family photos. They move to the app-managed holding area, not to the bin.",
      details: [
        { label: "Files", value: formatCount(ids.length) },
        { label: "Total size", value: formatBytes(selectedBytes) },
        { label: "Destination", value: "Digital Mess Cleaner quarantine" },
      ],
      note: "Reversible. Restore each file to its original folder from the Cleanup Centre.",
      primaryText: "Move to quarantine",
      secondaryText: "Keep files",
      tone: "caution",
    });
    if (!confirmed) return;

    setActionError(null);
    setNotice(null);
    setBusy(true);
    try {
      const result = await api.quarantineFiles(ids);
      bumpData();
      setSelected(new Set());
      setNotice(
        `Moved ${formatCount(result.moved)} ${pluralize(result.moved, "file")} to quarantine (${formatBytes(result.totalBytes)}).${
          result.failed > 0 ? ` ${result.failed} could not be moved.` : ""
        }`,
      );
    } catch (error) {
      setActionError(errorMessage(error));
    } finally {
      setBusy(false);
    }
  }

  const ageLabel = AGE_OPTIONS.find((option) => option.days === days)?.label ?? `${days} days`;

  return (
    <div className="page">
      <div className="page-heading">
        <div>
          <h1 className="page-title">Forgotten Files</h1>
          <p className="page-subtitle">
            Files you have not opened or changed in a long time
          </p>
        </div>
      </div>

      <div className="filter-row">
        <label className="filter-field">
          <span>Not opened in</span>
          <select value={days} onChange={(event) => setDays(Number(event.target.value))}>
            {AGE_OPTIONS.map((option) => (
              <option key={option.days} value={option.days}>
                {option.label}
              </option>
            ))}
          </select>
        </label>

        <label className="filter-field">
          <span>Size</span>
          <select
            value={minSize}
            onChange={(event) => setMinSize(Number(event.target.value))}
          >
            {SIZE_OPTIONS.map((option) => (
              <option key={option.bytes} value={option.bytes}>
                {option.label}
              </option>
            ))}
          </select>
        </label>

        {forgotten.data && (
          <span className="filter-meta">
            {formatCount(files.length)} {pluralize(files.length, "file")}
            {files.length === LIMIT ? ` (first ${formatCount(LIMIT)})` : ""} ·{" "}
            {formatBytes(totalBytes)}
          </span>
        )}
      </div>

      <p className="muted">
        “Forgotten” means no open or change recorded in the last {ageLabel}. It is a
        suggestion to review, never a verdict — open a file before deciding.
      </p>

      {actionError && <ErrorNotice message={actionError} />}
      {notice && <SuccessNotice message={notice} />}

      {forgotten.loading && !forgotten.data && (
        <LoadingNotice label="Reading access times from your local index…" />
      )}

      {forgotten.error && (
        <ErrorNotice
          message={forgotten.error}
          onRetry={forgotten.reload}
          desktopOnly={forgotten.errorCode === "not_desktop"}
        />
      )}

      {forgotten.data && files.length === 0 && (
        <EmptyState
          title="No forgotten files"
          description={`Nothing has sat untouched for ${ageLabel} at this size. Try a longer window or a smaller size.`}
          icon="clock"
        />
      )}

      {files.length > 0 && (
        <>
          <div className="stats-grid">
            <StatCard
              title="Files to review"
              value={formatCount(files.length)}
              subtitleOf={`Untouched for ${ageLabel}`}
            />
            <StatCard
              title="Space they use"
              value={formatBytes(totalBytes)}
              subtitleOf="Nothing removed until you approve"
            />
          </div>

          {selectedFiles.length > 0 && (
            <div className="selection-bar" role="region" aria-label="Selected files">
              <span>
                {formatCount(selectedFiles.length)}{" "}
                {pluralize(selectedFiles.length, "file")} selected ·{" "}
                {formatBytes(selectedBytes)}
              </span>
              <div className="selection-actions">
                <button
                  type="button"
                  className="btn btn-caution"
                  onClick={() => void quarantineSelected()}
                  disabled={busy}
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

          <div className="page-heading">
            <h2 className="section-title">
              <Clock size={16} aria-hidden="true" /> Oldest candidates first
            </h2>
          </div>

          <FileTable
            files={files}
            selected={selected}
            onToggle={toggle}
            onToggleAll={toggleAll}
            caption="Forgotten files"
          />
        </>
      )}
    </div>
  );
}
