import { useEffect, useId, useRef, useState } from "react";

export interface ReviewDetail {
  label: string;
  value: string;
}

export interface ReviewDialogProps {
  open?: boolean;
  onClose?: () => void;
  title?: string;
  message?: string;
  details?: ReviewDetail[];
  note?: string;
  /** When set, the primary action stays disabled until the user types this. */
  confirmPhrase?: string;
  primaryText?: string;
  secondaryText?: string;
  /** @deprecated Prefer tone="danger". Kept for existing call sites. */
  destructive?: boolean;
  tone?: "primary" | "caution" | "danger";
  busy?: boolean;
  onPrimary?: () => void;
  onSecondary?: () => void;
}

export function ReviewDialog({
  open = false,
  onClose = () => {},
  title = "",
  message = "",
  details = [],
  note,
  confirmPhrase,
  primaryText = "OK",
  secondaryText = "Cancel",
  destructive = false,
  tone,
  busy = false,
  onPrimary = () => {},
  onSecondary = () => {},
}: ReviewDialogProps) {
  const dialogRef = useRef<HTMLDivElement>(null);
  const titleId = useId();
  const messageId = useId();
  const confirmId = useId();
  const [typed, setTyped] = useState("");
  const resolvedTone = tone ?? (destructive ? "danger" : "primary");
  const phraseSatisfied = !confirmPhrase || typed.trim() === confirmPhrase;

  useEffect(() => {
    if (!open) {
      setTyped("");
      return;
    }

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
          'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])',
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
  }, [open, onClose]);

  if (!open) return null;

  const primaryClass =
    resolvedTone === "danger"
      ? "btn btn-danger"
      : resolvedTone === "caution"
        ? "btn btn-caution"
        : "btn btn-primary";

  return (
    <div className="dialog-backdrop">
      <div
        ref={dialogRef}
        className="dialog review-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        aria-describedby={messageId}
        tabIndex={-1}
      >
        <div className="dialog-header">
          <h3 id={titleId} className="dialog-title">
            {title}
          </h3>
          <button
            type="button"
            onClick={onClose}
            className="dialog-close"
            aria-label="Close dialog"
          >
            ✕
          </button>
        </div>

        <div className="dialog-body">
          <p id={messageId}>{message}</p>

          {details.length > 0 && (
            <dl className="review-details">
              {details.map((detail) => (
                <div key={detail.label} className="review-detail">
                  <dt>{detail.label}</dt>
                  <dd>{detail.value}</dd>
                </div>
              ))}
            </dl>
          )}

          {note && <p className="review-note">{note}</p>}

          {confirmPhrase && (
            <div className="review-confirm">
              <label htmlFor={confirmId}>
                Type <strong>{confirmPhrase}</strong> to continue
              </label>
              <input
                id={confirmId}
                type="text"
                value={typed}
                autoComplete="off"
                onChange={(event) => setTyped(event.target.value)}
              />
            </div>
          )}
        </div>

        <div className="dialog-footer">
          <button
            type="button"
            onClick={() => {
              onSecondary();
              onClose();
            }}
            className="btn btn-secondary"
            disabled={busy}
          >
            {secondaryText}
          </button>
          <button
            type="button"
            onClick={onPrimary}
            className={primaryClass}
            disabled={busy || !phraseSatisfied}
          >
            {busy ? "Working…" : primaryText}
          </button>
        </div>
      </div>
    </div>
  );
}
