import { EmptyState } from "../components/EmptyState";
import { ProgressBar } from "../components/ProgressBar";

export function FileHealthPage() {
  return (
    <div className="page">
      <h1 className="page-title">File Health</h1>
      <p className="page-subtitle">Check document integrity and application leftovers</p>

      <div className="health-checks">
        <div className="check-item">
          <h3>Document Integrity</h3>
          <p>Find corrupted or unreadable documents</p>
          <ProgressBar progress={80} label="80% checked" />
        </div>

        <div className="check-item">
          <h3>Application Leftovers</h3>
          <p>Find leftover data from uninstalled apps</p>
          <ProgressBar progress={60} label="60% checked" />
        </div>

        <div className="check-item">
          <h3>Broken Shortcuts</h3>
          <p>Find deleted file shortcuts</p>
          <ProgressBar progress={40} label="40% checked" />
        </div>
      </div>

      <div className="health-summary">
        <h3>Overall Health: Good</h3>
        <p>2 of 15 issues found. All critical issues resolved.</p>
        <button className="btn">Full Scan</button>
      </div>

      <EmptyState
        title="No issues found"
        description="All files and applications are healthy."
        icon="check-circle"
      />
    </div>
  );
}