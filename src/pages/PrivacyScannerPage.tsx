import { EmptyState } from "../components/EmptyState";

export function PrivacyScannerPage() {
  return (
    <div className="page">
      <h1 className="page-title">Privacy Scanner</h1>
      <p className="page-subtitle">Identify sensitive and personal data</p>

      <div className="privacy-risks">
        <div className="risk-card">
          <h3>Sensitive Documents</h3>
          <p>Files containing personal information, financial data, or passwords</p>
          <p className="risk-count">3 files found</p>
          <button className="btn btn-caution">Review</button>
        </div>

        <div className="risk-card">
          <h3>Potential Passwords</h3>
          <p>Documents that may contain stored passwords</p>
          <p className="risk-count">1 file found</p>
          <button className="btn btn-caution">Review</button>
        </div>

        <div className="risk-card">
          <h3>Aadhaar Numbers</h3>
          <p>Indian identity numbers potentially exposed</p>
          <p className="risk-count">1 found</p>
          <button className="btn btn-caution">Review</button>
        </div>
      </div>

      <div className="privacy-actions">
        <button className="btn">Scan Now</button>
        <button className="btn btn-danger">Purge All Findings</button>
      </div>

      <EmptyState
        title="No risks found"
        description="Your files appear to be privacy-safe."
        icon="shield-check"
      />
    </div>
  );
}