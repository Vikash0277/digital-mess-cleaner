import { EmptyState } from "../components/EmptyState";

export function SmartOrganizerPage() {
  return (
    <div className="page">
      <h1 className="page-title">Smart Organizer</h1>
      <p className="page-subtitle">Organize files with intelligent rules</p>

      <div className="organizer-rules">
        <div className="rule-card">
          <h3>Auto-organize Downloads</h3>
          <p>Automatically sort downloaded files by type</p>
          <button className="btn">Set Up Rule</button>
        </div>

        <div className="rule-card">
          <h3>Smart Renaming</h3>
          <p>Rename files based on their content</p>
          <button className="btn">Configure</button>
        </div>

        <div className="rule-card">
          <h3>Bulk Organization</h3>
          <p>Organize thousands of files at once</p>
          <button className="btn btn-caution">Preview Changes</button>
        </div>
      </div>

      <EmptyState
        title="No rules configured"
        description="Set up organization rules to automatically sort your files."
        icon="layout"
        buttonText="Create First Rule"
      />
    </div>
  );
}