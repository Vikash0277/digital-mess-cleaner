import { EmptyState } from "../components/EmptyState";

export function AutomationCentrePage() {
  return (
    <div className="page">
      <h1 className="page-title">Automation Centre</h1>
      <p className="page-subtitle">Automate repetitive file tasks</p>

      <div className="automation-rules">
        <div className="rule">
          <div className="rule-header">
            <span>Auto-sort Downloads</span>
            <span>Every 15 min</span>
          </div>
          <div className="rule-description">
            Automatically sort downloaded files by type into appropriate folders.
          </div>
        </div>

        <div className="rule">
          <div className="rule-header">
            <span>Weekly Storage Scan</span>
            <span>Every Sunday</span>
          </div>
          <div className="rule-description">
            Run storage analysis and report every Sunday at 9 AM.
          </div>
        </div>
      </div>

      <div className="automation-history">
        <h3>Automation History</h3>
        <ul>
          <li>2 hours ago: Auto-sorted 23 files</li>
          <li>Yesterday: Weekly scan completed</li>
          <li>3 days ago: No new duplicates found</li>
        </ul>
      </div>

      <div className="auto-apply-settings">
        <label>
          <input type="checkbox" defaultChecked />
          <span>Enable auto-apply</span>
        </label>
        <p>
          Automatically apply suggestions when conditions are met. Otherwise, show notifications.
        </p>
      </div>

      <EmptyState
        title="No automation rules"
        description="Set up automation rules to save time on repetitive tasks."
        icon="zap"
        buttonText="Create First Rule"
      />
    </div>
  );
}