import { EmptyState } from "../components/EmptyState";

export function ComparisonPage() {
  return (
    <div className="page">
      <h1 className="page-title">File Comparison</h1>
      <p className="page-subtitle">Compare two files side by side</p>

      <div className="comparison-interface">
        <div className="comparison-panel left">
          <h3>File 1</h3>
          <div className="file-info">
            <span>Name: document.pdf</span>
            <span>Size: 45 KB</span>
            <span>Pages: 12</span>
          </div>
          <div className="content-preview">
            <p>...document content preview...</p>
          </div>
        </div>

        <div className="comparison-panel right">
          <h3>File 2</h3>
          <div className="file-info">
            <span>Name: document_v2.pdf</span>
            <span>Size: 48 KB</span>
            <span>Pages: 13</span>
          </div>
          <div className="content-preview">
            <p>...document content preview...</p>
          </div>
        </div>
      </div>

      <div className="comparison-highlights">
        <h3>Differences</h3>
        <ul>
          <li>Page count differs: 12 vs 13 pages</li>
          <li>File size differs: 45 KB vs 48 KB</li>
          <li>Last modified: different dates</li>
        </ul>
      </div>

      <div className="action-controls">
        <button className="btn">Run Full Comparison</button>
        <button className="btn btn-primary">Generate Report</button>
      </div>

      <EmptyState
        title="Select files to compare"
        description="Choose two files to compare their content and metadata."
        icon="arrows-between-files"
        buttonText="Select Files"
      />
    </div>
  );
}
