import { EmptyState } from "../components/EmptyState";

export function ExtractionPage() {
  return (
    <div className="page">
      <h1 className="page-title">Document Extraction</h1>
      <p className="page-subtitle">Extract key data from documents</p>

      <div className="extraction-interface">
        <div className="extraction-options">
          <div className="extraction-option">
            <input type="radio" name="extraction-type" id="invoice" />
            <label htmlFor="invoice">Extract Invoice Data</label>
          </div>
          <div className="extraction-option">
            <input type="radio" name="extraction-type" id="receipt" />
            <label htmlFor="receipt">Extract Receipt Data</label>
          </div>
          <div className="extraction-option">
            <input type="radio" name="extraction-type" id="resume" defaultChecked />
            <label htmlFor="resume">Extract Resume Data</label>
          </div>
        </div>

        <div className="file-selection">
          <h3>Select Document</h3>
          <p>Choose a PDF or document file to extract data from.</p>
        </div>

        <div className="extraction-results">
          <h3>Extracted Data</h3>
          <div className="extraction-fields">
            <div className="extraction-field">
              <span>Invoice Number:</span>
              <span>INV-2024-001</span>
            </div>
            <div className="extraction-field">
              <span>Date:</span>
              <span>2024-01-15</span>
            </div>
            <div className="extraction-field">
              <span>Total:</span>
              <span>$1,250.00</span>
            </div>
          </div>

          <div className="extraction-fields">
            <div className="extraction-field">
              <span>Vendor:</span>
              <span>ABC Corp</span>
            </div>
            <div className="extraction-field">
              <span>Tax ID:</span>
              <span>12-3456789</span>
            </div>
          </div>
        </div>
      </div>

      <div className="action-controls">
        <button className="btn">Scan New Document</button>
        <button className="btn btn-primary">Extract Data</button>
      </div>

      <EmptyState
        title="No document selected"
        description="Select a PDF or document to extract data."
        icon="file-text"
        buttonText="Choose File"
      />
    </div>
  );
}
