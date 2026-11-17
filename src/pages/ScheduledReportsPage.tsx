import { EmptyState } from "../components/EmptyState";

export function ScheduledReportsPage() {
  return (
    <div className="page">
      <h1 className="page-title">Scheduled Reports</h1>
      <p className="page-subtitle">Automated storage reports on a schedule</p>

      <div className="reports-interface">
        <div className="report-schedules">
          <h3>Report Schedules</h3>
          <div className="schedule-item">
            <div className="schedule-info">
              <span>Daily Storage Summary</span>
              <span>Every day at 8:00 AM</span>
            </div>
            <div className="schedule-status">
              <span className="status-active">Enabled</span>
              <span>PDF format</span>
            </div>
          </div>

          <div className="schedule-item">
            <div className="schedule-info">
              <span>Weekly Duplicate Report</span>
              <span>Every Sunday at 6:00 PM</span>
            </div>
            <div className="schedule-status">
              <span className="status-active">Enabled</span>
              <span>HTML format</span>
            </div>
          </div>

          <div className="schedule-item">
            <div className="schedule-info">
              <span>Monthly Storage Growth</span>
              <span>1st of month at 12:00 AM</span>
            </div>
            <div className="schedule-status">
              <span className="status-inactive">Disabled</span>
              <span>JSON format</span>
            </div>
          </div>
        </div>

        <div className="add-report">
          <h3>Create Report Schedule</h3>
          <div className="form-group">
            <label htmlFor="report-name">Report Name</label>
            <input id="report-name" type="text" placeholder="e.g., Weekly Storage Report" />
          </div>
          <div className="form-group">
            <label htmlFor="report-schedule">Schedule</label>
            <select id="report-schedule">
              <option>Daily</option>
              <option>Weekly</option>
              <option>Monthly</option>
            </select>
          </div>
          <div className="form-group">
            <label htmlFor="report-format">Format</label>
            <select id="report-format">
              <option>PDF</option>
              <option>HTML</option>
              <option>JSON</option>
              <option>CSV</option>
            </select>
          </div>
          <button className="btn">Create Schedule</button>
        </div>
      </div>

      <div className="report-history">
        <h3>Recent Reports</h3>
        <div className="history-list">
          <div className="history-item">
            <span>Daily Storage Summary</span>
            <span>Today, 8:00 AM</span>
            <span>Generated</span>
          </div>
          <div className="history-item">
            <span>Weekly Duplicate Report</span>
            <span>Yesterday, 6:00 PM</span>
            <span>Generated</span>
          </div>
          <div className="history-item">
            <span>Monthly Storage Growth</span>
            <span>Last month, 12:00 AM</span>
            <span>Failed</span>
          </div>
        </div>
      </div>

      <EmptyState
        title="No schedules configured"
        description="Set up automated reports to track storage usage over time."
        icon="clock"
        buttonText="Create First Schedule"
      />
    </div>
  );
}