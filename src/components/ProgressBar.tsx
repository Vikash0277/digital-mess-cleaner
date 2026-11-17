export interface ProgressBarProps {
  progress: number;
  label?: string;
  showValue?: boolean;
}

export function ProgressBar({ progress, label, showValue = false }: ProgressBarProps) {
  const percentage = Math.round(Math.max(0, Math.min(100, progress)));

  return (
    <div className="progress">
      <div
        className="progress-track"
        role="progressbar"
        aria-valuenow={percentage}
        aria-valuemin={0}
        aria-valuemax={100}
        aria-label={label}
      >
        <div className="progress-bar" style={{ width: `${percentage}%` }} />
      </div>
      {(label || showValue) && (
        <span className="progress-label">{label ?? `${percentage}%`}</span>
      )}
    </div>
  );
}
