import { ConfidenceBadge } from "../components/ConfidenceBadge";
import { EmptyState } from "../components/EmptyState";
import { ProgressBar } from "../components/ProgressBar";

export function FolderSuggestionsPage() {
  return (
    <div className="page">
      <h1 className="page-title">Folder Suggestions</h1>
      <p className="page-subtitle">Smart folder creation recommendations</p>

      <div className="suggestions-container">
        <div className="usage-stats">
          <h3>Usage Patterns</h3>
          <div className="stat-bar">
            <span>Documents</span>
            <ProgressBar progress={62} />
            <span>45 files</span>
          </div>
          <div className="stat-bar">
            <span>Images</span>
            <ProgressBar progress={38} />
            <span>30 files</span>
          </div>
          <div className="stat-bar">
            <span>Videos</span>
            <ProgressBar progress={12} />
            <span>15 files</span>
          </div>
        </div>

        <div className="suggestions-list">
          <h3>Suggested Folders</h3>
          <div className="suggestion-item">
            <div className="suggestion-icon">📁</div>
            <div className="suggestion-details">
              <h4>Documents Folder</h4>
              <p>Recommended for: PDFs, Word docs, spreadsheets</p>
              <ConfidenceBadge confidence={0.95} />
            </div>
          </div>

          <div className="suggestion-item">
            <div className="suggestion-icon">🖼️</div>
            <div className="suggestion-details">
              <h4>Images Folder</h4>
              <p>Recommended for: Photos, screenshots, graphics</p>
              <ConfidenceBadge confidence={0.85} />
            </div>
          </div>

          <div className="suggestion-item">
            <div className="suggestion-icon">📹</div>
            <div className="suggestion-details">
              <h4>Videos Folder</h4>
              <p>Recommended for: Movies, recordings, presentations</p>
              <ConfidenceBadge confidence={0.75} />
            </div>
          </div>

          <div className="suggestion-item">
            <div className="suggestion-icon">💼</div>
            <div className="suggestion-details">
              <h4>Projects Folder</h4>
              <p>Recommended for: Project files, presentations</p>
              <ConfidenceBadge confidence={0.65} />
            </div>
          </div>
        </div>
      </div>

      <div className="action-controls">
        <button className="btn">Create Suggested Folders</button>
        <button className="btn btn-primary">Review & Organise</button>
      </div>

      <EmptyState
        title="No suggestions yet"
        description="Browse files to generate personalised folder recommendations."
        icon="folder-add"
        buttonText="Generate Suggestions"
      />
    </div>
  );
}
