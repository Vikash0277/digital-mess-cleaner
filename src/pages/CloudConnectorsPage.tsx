import { EmptyState } from "../components/EmptyState";

export function CloudConnectorsPage() {
  return (
    <div className="page">
      <h1 className="page-title">Cloud Connectors</h1>
      <p className="page-subtitle">Connect cloud storage accounts</p>

      <div className="connectors-interface">
        <div className="connectors-list">
          <div className="connector-item">
            <div className="connector-info">
              <span>Google Drive</span>
              <span>john.doe@gmail.com</span>
            </div>
            <div className="connector-status">
              <span className="status-connected">Connected</span>
              <span>15 GB used of 15 GB</span>
            </div>
            <div className="connector-actions">
              <button className="btn">Sync Now</button>
              <button className="btn btn-primary">Disconnect</button>
            </div>
          </div>

          <div className="connector-item">
            <div className="connector-info">
              <span>OneDrive</span>
              <span>john.doe@outlook.com</span>
            </div>
            <div className="connector-status">
              <span className="status-connected">Connected</span>
              <span>100 GB used of 1 TB</span>
            </div>
            <div className="connector-actions">
              <button className="btn">Sync Now</button>
              <button className="btn btn-primary">Disconnect</button>
            </div>
          </div>

          <div className="connector-item">
            <div className="connector-info">
              <span>Dropbox</span>
              <span>Not connected</span>
            </div>
            <div className="connector-status">
              <span className="status-disconnected">Not connected</span>
              <span>No files synced</span>
            </div>
            <div className="connector-actions">
              <button className="btn">Connect</button>
              <button className="btn btn-primary" disabled>Disconnect</button>
            </div>
          </div>
        </div>
      </div>

      <div className="add-connector">
        <h3>Add Cloud Connector</h3>
        <div className="connector-options">
          <div className="connector-option">
            <div className="connector-icon">📁</div>
            <span>Google Drive</span>
          </div>
          <div className="connector-option">
            <div className="connector-icon">☁️</div>
            <span>OneDrive</span>
          </div>
          <div className="connector-option">
            <div className="connector-icon">📦</div>
            <span>Dropbox</span>
          </div>
        </div>
        <p>Connect cloud storage to scan and manage remote files.</p>
      </div>

      <div className="sync-status">
        <h3>Sync Status</h3>
        <div className="sync-item">
          <span>Google Drive</span>
          <span>Last synced: 2 hours ago</span>
          <span className="status-success">Synced</span>
        </div>
        <div className="sync-item">
          <span>OneDrive</span>
          <span>Last synced: 5 hours ago</span>
          <span className="status-success">Synced</span>
        </div>
      </div>

      <EmptyState
        title="No cloud connectors configured"
        description="Connect cloud storage accounts to scan and manage remote files."
        icon="cloud"
        buttonText="Add Connector"
      />
    </div>
  );
}