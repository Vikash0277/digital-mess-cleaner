import { EmptyState } from "../components/EmptyState";

export function SourceReferencesPage() {
  return (
    <div className="page">
      <h1 className="page-title">Source References</h1>
      <p className="page-subtitle">Citations and references for extracted data</p>

      <div className="references-list">
        <div className="reference-item">
          <div className="reference-header">
            <span>Invoice #INV-2024-001</span>
            <span>2024-01-15</span>
          </div>
          <div className="reference-details">
            <p>Extracted from: contracts/invoice_2024_q1.pdf</p>
            <p>Page: 1 of 2</p>
            <p>Method: OCR + Pattern Matching</p>
          </div>
        </div>

        <div className="reference-item">
          <div className="reference-header">
            <span>Resume - John Doe</span>
            <span>2023-09-01</span>
          </div>
          <div className="reference-details">
            <p>Extracted from: john_doe_resume.pdf</p>
            <p>Page: 1 of 2</p>
            <p>Method: Text Extraction + Keyword Detection</p>
          </div>
        </div>

        <div className="reference-item">
          <div className="reference-header">
            <span>Receipt #REC-2024-045</span>
            <span>2024-03-20</span>
          </div>
          <div className="reference-details">
            <p>Extracted from: purchase_receipts/2024_q1.pdf</p>
            <p>Page: 1 of 1</p>
            <p>Method: Template-Based Extraction</p>
          </div>
        </div>
      </div>

      <div className="reference-actions">
        <button className="btn">Refresh References</button>
        <button className="btn btn-primary">Export References</button>
      </div>

      <EmptyState
        title="No references yet"
        description="Extract data first to generate source references."
        icon="file-text"
        buttonText="Extract Data First"
      />
    </div>
  );
}
