import { EmptyState } from "../components/EmptyState";

export function RulesEnginePage() {
  return (
    <div className="page">
      <h1 className="page-title">Rules Engine</h1>
      <p className="page-subtitle">Manage automation rules for your files</p>

      <div className="rules-management">
        <div className="rule-create">
          <h3>Create New Rule</h3>
          <div className="form-group">
            <label htmlFor="rule-name">Rule Name</label>
            <input id="rule-name" type="text" placeholder="e.g., Auto-sort Downloads" />
          </div>
          <div className="form-group">
            <label htmlFor="rule-condition">Condition</label>
            <input id="rule-condition" type="text" placeholder="e.g., .pdf files" />
          </div>
          <div className="form-group">
            <label htmlFor="rule-action">Action</label>
            <input id="rule-action" type="text" placeholder="e.g., move to Documents folder" />
          </div>
          <button className="btn btn-primary">Create Rule</button>
        </div>

        <div className="rules-list">
          <h3>Existing Rules</h3>
          <div className="rule-item">
            <div className="rule-info">
              <span>Auto-sort Downloads</span>
              <span>Every 15 min</span>
            </div>
            <div className="rule-actions">
              <button className="btn">Edit</button>
              <button className="btn btn-danger">Delete</button>
            </div>
          </div>

          <div className="rule-item">
            <div className="rule-info">
              <span>Weekly Scan</span>
              <span>Sundays at 9 AM</span>
            </div>
            <div className="rule-actions">
              <button className="btn">Edit</button>
              <button className="btn btn-danger">Delete</button>
            </div>
          </div>
        </div>
      </div>

      <EmptyState
        title="No rules configured"
        description="Create your first rule to automate file tasks."
        icon="zap"
        buttonText="Create First Rule"
      />
    </div>
  );
}