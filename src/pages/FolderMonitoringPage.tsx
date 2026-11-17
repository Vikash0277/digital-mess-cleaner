import { EmptyState } from "../components/EmptyState";

export function FolderMonitoringPage() {
  return (
    <div className="page">
      <h1 className="page-title">Folder Monitoring</h1>
      <p className="page-subtitle">Watch folders for changes automatically</p>

      <div className="monitoring-interface">
        <div className="watched-folders">
          <h3>Watched Folders</h3>
          <div className="folder-item">
            <div className="folder-info">
              <span>Downloads</span>
              <span>C:\Users\Public\Downloads</span>
            </div>
            <div className="folder-status">
              <span className="status-active">Active</span>
              <span>Every 15 min</span>
            </div>
          </div>

          <div className="folder-item">
            <div className="folder-info">
              <span>Documents</span>
              <span>C:\Users\Public\Documents</span>
            </div>
            <div className="folder-status">
              <span className="status-active">Active</span>
              <span>Hourly</span>
            </div>
          </div>

          <div className="folder-item">
            <div className="folder-info">
              <span>Desktop</span>
              <span>C:\Users\Public\Desktop</span>
            </div>
            <div className="folder-status">
              <span className="status-inactive">Inactive</span>
              <span>Disabled</span>
            </div>
          </div>
        </div>

        <div className="add-folder">
          <h3>Add Folder to Monitor</h3>
          <div className="form-group">
            <label htmlFor="watch-path">Folder Path</label>
            <input id="watch-path" type="text" placeholder="Select or enter folder path" />
          </div>
          <div className="form-group">
            <label htmlFor="watch-frequency">Scan Frequency</label>
            <select id="watch-frequency">
              <option>Every 15 minutes</option>
              <option>Hourly</option>
              <option>Daily</option>
            </select>
          </div>
          <button className="btn">Add Folder</button>
        </div>
      </div>

      <div className="recent-changes">
        <h3>Recent Changes</h3>
        <div className="changes-list">
          <div className="change-item">
            <span>New file added</span>
            <span>report.pdf</span>
            <span>2 minutes ago</span>
          </div>
          <div className="change-item">
            <span>File modified</span>
            <span>budget.xlsx</span>
            <span>15 minutes ago</span>
          </div>
          <div className="change-item">
            <span>File deleted</span>
            <span>draft.docx</span>
            <span>1 hour ago</span>
          </div>
        </div>
      </div>

      <EmptyState
        title="No folders monitored"
        description="Add folders to monitor for automatic change detection."
        icon="folder-watch"
        buttonText="Add First Folder"
      />
    </div>
  );
}