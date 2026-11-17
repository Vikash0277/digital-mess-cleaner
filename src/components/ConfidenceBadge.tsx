export function ConfidenceBadge({ confidence }: { confidence: number }) {
  const percentage = Math.round(Math.max(0, Math.min(1, confidence)) * 100);
  const level = percentage >= 85 ? "high" : percentage >= 60 ? "medium" : "low";
  const label =
    level === "high"
      ? "High confidence"
      : level === "medium"
        ? "Medium confidence"
        : "Low confidence — verify";

  return (
    <span
      className="confidence"
      data-level={level}
      title={`${percentage}% confidence`}
    >
      {label} · {percentage}%
    </span>
  );
}
