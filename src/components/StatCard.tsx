import { ConfidenceBadge } from "./ConfidenceBadge";

export interface StatCardProps {
  title: string;
  value: string;
  subtitleOf?: string;
  /** Only pass a confidence for inferred values, not for exact counts. */
  confidence?: number;
}

export function StatCard({ title, value, subtitleOf, confidence }: StatCardProps) {
  return (
    <div className="card stat-card">
      <h3>{title}</h3>
      <p className="value">{value}</p>
      {subtitleOf && <p className="subtitle">{subtitleOf}</p>}
      {confidence !== undefined && <ConfidenceBadge confidence={confidence} />}
    </div>
  );
}
