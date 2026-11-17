import { EmptyState } from "../components/EmptyState";

export function MigrationAssistantPage() {
  return (
    <div className="page">
      <h1 className="page-title">Migration Assistant</h1>
      <p className="page-subtitle">Move to a new computer</p>

      <div className="migration-steps">
        <div className="step">
          <span>1</span>
          <h3>Identify Important Files</h3>
          <p>Scan your current drive for documents, photos, and apps</p>
        </div>

        <div className="step">
          <span>2</span>
          <h3>Organize and Detect Duplicates</h3>
          <p>Remove duplicates and sort files by type</p>
        </div>

        <div className="step">
          <span>3</span>
          <h3>Prepare Migration Checklist</h3>
          <p>Generate a list of files to transfer</p>
        </div>

        <div className="step">
          <span>4</span>
          <h3>Transfer and Verify</h3>
          <p>Move files to new computer and verify integrity</p>
        </div>
      </div>

      <div className="migration-status">
        <h3>Migration Status: Not started</h3>
        <p>No files have been identified for migration yet.</p>
        <button className="btn btn-primary">Start Scan</button>
      </div>

      <EmptyState
        title="Start migration"
        description="Begin the migration process to identify your important files."
        icon="download"
        buttonText="Start Scan"
      />
    </div>
  );
}