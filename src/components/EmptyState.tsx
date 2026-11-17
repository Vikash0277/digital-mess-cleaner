import { Inbox } from "lucide-react";
import type { ReactNode } from "react";

export function EmptyState({
  title,
  description,
  buttonText,
  icon,
  onAction,
  actionDisabled = false,
}: {
  title: string;
  description: string;
  buttonText?: string;
  icon?: ReactNode;
  onAction?: () => void;
  actionDisabled?: boolean;
}) {
  return (
    <div className="empty-state">
      <div className="empty-state-icon" aria-hidden="true">
        {typeof icon === "string" || icon === undefined ? <Inbox /> : icon}
      </div>
      <h3 className="empty-state-title">{title}</h3>
      <p className="empty-state-text">{description}</p>
      {buttonText && (
        <button
          type="button"
          className="btn btn-default"
          onClick={onAction}
          disabled={actionDisabled}
        >
          {buttonText}
        </button>
      )}
    </div>
  );
}
