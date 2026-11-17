import { EmptyState } from "../components/EmptyState";

export function SummariesPage() {
  return (
    <div className="page">
      <h1 className="page-title">Document Summaries</h1>
      <p className="page-subtitle">AI-generated summaries of your files</p>

      <div className="summaries-list">
        <div className="summary-item">
          <div className="summary-preview">
            <p>...document content would appear here...</p>
            <p>
              <strong>Summary:</strong> This document appears to be a project report...
            </p>
          </div>
          <div className="summary-meta">
            <span>Type: Report</span>
            <span>Pages: 12</span>
            <span>Generated: 2 minutes ago</span>
          </div>
        </div>

        <div className="summary-item">
          <div className="summary-preview">
            <p>...another document summary...</p>
            <p>
              <strong>Summary:</strong> This is a meeting notes file from the team...
            </p>
          </div>
          <div className="summary-meta">
            <span>Type: Meeting Notes</span>
            <span>Pages: 3</span>
            <span>Generated: 5 minutes ago</span>
          </div>
        </div>
      </div>

      <div className="action-controls">
        <button className="btn">Generate Summaries for Selected Files</button>
        <button className="btn btn-primary">Full Drive Scan</button>
      </div>

      <EmptyState
        title="No summaries yet"
        description="Files will appear here after analysis."
        icon="file-text"
        buttonText="Start Analysis"
      />
    </div>
  );
}
