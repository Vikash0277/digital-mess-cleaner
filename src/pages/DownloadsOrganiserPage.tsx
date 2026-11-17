import { EmptyState } from "../components/EmptyState";

export function DownloadsOrganiserPage() {
  return (
    <div className="page">
      <h1 className="page-title">Downloads Organiser</h1>
      <p className="page-subtitle">Auto-sort your downloaded files</p>

      <div className="downloads-management">
        <div className="stats-overview">
          <div className="stat-item">
            <div className="stat-number">0</div>
            <div className="stat-label">Files this session</div>
          </div>
          <div className="stat-item">
            <div className="stat-number">0</div>
            <div className="stat-label">Total size</div>
          </div>
        </div>

        <div className="rules-section">
          <h3>Organising Rules</h3>
          <p>No rules configured yet. <button className="btn">Set Up Rules</button></p>
        </div>

        <div className="recent-imports">
          <h3>Recent Downloads</h3>
          <div className="import-item">
            <div className="import-icon">
              <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor">
                <path d="M21 15v5a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-5" />
                <path d="M17 5H7a2 2 0 0 0-2 2v4" />
                <path d="M7 11l5 5 5-5" />
                <path d="M7 16l5 5 5-5" />
              </svg>
            </div>
            <div className="import-info">
              <span>document.pdf</span>
              <span>45 KB</span>
            </div>
          </div>

          <div className="import-item">
            <div className="import-icon">
              <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor">
                <path d="M21 15v5a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-5" />
                <path d="M17 5H7a2 2 0 0 0-2 2v4" />
                <path d="M7 11l5 5 5-5" />
                <path d="M7 16l5 5 5-5" />
              </svg>
            </div>
            <div className="import-info">
              <span>image.png</span>
              <span>120 KB</span>
            </div>
          </div>
        </div>
      </div>

      <EmptyState
        title="No downloads yet"
        description="Download files will appear here after you browse."
        icon="download"
      />
    </div>
  );
}