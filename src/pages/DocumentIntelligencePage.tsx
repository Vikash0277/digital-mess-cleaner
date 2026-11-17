import { EmptyState } from "../components/EmptyState";

export function DocumentIntelligencePage() {
  return (
    <div className="page">
      <h1 className="page-title">Document Intelligence</h1>
      <p className="page-subtitle">Extract insights from your documents</p>

      <div className="doc-intel">
        <div className="analysis-cards">
          <div className="card">
            <h3>Text Extraction</h3>
            <p>Extract text from PDFs and documents</p>
          </div>
          <div className="card">
            <h3>Document Classification</h3>
            <p>Auto-categorize your files</p>
          </div>
          <div className="card">
            <h3>Field Extraction</h3>
            <p>Extract dates, amounts, and identifiers</p>
          </div>
        </div>

        <div className="preview-panel">
          <h3>Preview</h3>
          <p>Document preview would appear here</p>
        </div>
      </div>

      <EmptyState
        title="No documents analyzed"
        description="Scan a drive to begin document analysis."
        icon="file-text"
      />
    </div>
  );
}