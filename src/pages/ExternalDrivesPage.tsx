import { EmptyState } from "../components/EmptyState";

export function ExternalDrivesPage() {
  return (
    <div className="page">
      <h1 className="page-title">External Drives</h1>
      <p className="page-subtitle">Manage external drives and removable storage</p>

      <div className="drives-interface">
        <div className="drives-list">
          <div className="drive-item">
            <div className="drive-info">
              <span>USB Drive (E:)</span>
              <span>Kingston DataTraveler</span>
            </div>
            <div className="drive-stats">
              <span className="status-connected">Connected</span>
              <div className="usage">
                <span>64 GB used of 128 GB</span>
              </div>
            </div>
            <div className="drive-actions">
              <button className="btn">Scan Drive</button>
              <button className="btn btn-primary">Eject</button>
            </div>
          </div>

          <div className="drive-item">
            <div className="drive-info">
              <span>External HDD (F:)</span>
              <span>Seagate Backup Plus</span>
            </div>
            <div className="drive-stats">
              <span className="status-connected">Connected</span>
              <div className="usage">
                <span>850 GB used of 2 TB</span>
              </div>
            </div>
            <div className="drive-actions">
              <button className="btn">Scan Drive</button>
              <button className="btn btn-primary">Eject</button>
            </div>
          </div>

          <div className="drive-item">
            <div className="drive-info">
              <span>SD Card (G:)</span>
              <span>SanDisk Extreme</span>
            </div>
            <div className="drive-stats">
              <span className="status-disconnected">Disconnected</span>
              <div className="usage">
                <span>Not available</span>
              </div>
            </div>
            <div className="drive-actions">
              <button className="btn" disabled>Scan Drive</button>
              <button className="btn btn-primary" disabled>Eject</button>
            </div>
          </div>
        </div>
      </div>

      <div className="drive-actions-global">
        <h3>Drive Actions</h3>
        <div className="action-buttons">
          <button className="btn">Scan All Connected Drives</button>
          <button className="btn btn-primary">Refresh Drive List</button>
        </div>
      </div>

      <div className="recent-drive-scans">
        <h3>Recent Drive Scans</h3>
        <div className="scan-list">
          <div className="scan-item">
            <span>USB Drive (E:)</span>
            <span>1245 files found</span>
            <span>2 hours ago</span>
          </div>
          <div className="scan-item">
            <span>External HDD (F:)</span>
            <span>8900 files found</span>
            <span>Yesterday</span>
          </div>
        </div>
      </div>

      <EmptyState
        title="No drives connected"
        description="Connect an external drive to scan and manage its contents."
        icon="hard-drive"
        buttonText="Refresh"
      />
    </div>
  );
}