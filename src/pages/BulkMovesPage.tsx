import { EmptyState } from "../components/EmptyState";

export function BulkMovesPage() {
  return (
    <div className="page">
      <h1 className="page-title">Bulk Moves</h1>
      <p className="page-subtitle">Select and move multiple files at once</p>

      <div className="bulk-moves-interface">
        <div className="selection-panel">
          <h3>Select Files</h3>
          <div className="file-list">
            <div className="file-item">
              <input type="checkbox" id="file1" name="file1" />
              <label htmlFor="file1" aria-label="document.pdf">
                <div className="file-icon">
                  <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor">
                    <path d="M21 15v5a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-5" />
                    <path d="M17 5H7a2 2 0 0 0-2 2v4" />
                    <path d="M7 11l5 5 5-5" />
                    <path d="M7 16l5 5 5-5" />
                  </svg>
                </div>
                <div className="file-details">
                  <span>document.pdf</span>
                  <span>45 KB</span>
                </div>
              </label>
            </div>

            <div className="file-item">
              <input type="checkbox" id="file2" name="file2" />
              <label htmlFor="file2" aria-label="image.png">
                <div className="file-icon">
                  <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor">
                    <path d="M21 15v5a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-5" />
                    <path d="M17 5H7a2 2 0 0 0-2 2v4" />
                    <path d="M7 11l5 5 5-5" />
                    <path d="M7 16l5 5 5-5" />
                  </svg>
                </div>
                <div className="file-details">
                  <span>image.png</span>
                  <span>120 KB</span>
                </div>
              </label>
            </div>

            <div className="file-item">
              <input type="checkbox" id="file3" name="file3" />
              <label htmlFor="file3" aria-label="presentation.pptx">
                <div className="file-icon">
                  <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor">
                    <path d="M21 15v5a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-5" />
                    <path d="M17 5H7a2 2 0 0 0-2 2v4" />
                    <path d="M7 11l5 5 5-5" />
                    <path d="M7 16l5 5 5-5" />
                  </svg>
                </div>
                <div className="file-details">
                  <span>presentation.pptx</span>
                  <span>250 KB</span>
                </div>
              </label>
            </div>
          </div>

          <div className="selection-count">
            <span>0 files selected</span>
            <span>0 KB</span>
          </div>
        </div>

        <div className="destination-panel">
          <h3>Destination</h3>
          <div className="destination-options">
            <div className="dest-option">
              <input type="radio" name="destination" id="dest-docs" />
              <label htmlFor="dest-docs">
                <span>Documents folder</span>
                <span>45 KB total</span>
              </label>
            </div>
            <div className="dest-option">
              <input type="radio" name="destination" id="dest-images" />
              <label htmlFor="dest-images">
                <span>Images folder</span>
                <span>120 KB total</span>
              </label>
            </div>
            <div className="dest-option">
              <input type="radio" name="destination" id="dest-desktop" />
              <label htmlFor="dest-desktop">
                <span>Desktop</span>
                <span>200 KB total</span>
              </label>
            </div>
          </div>
        </div>

        <div className="action-controls">
          <button className="btn" disabled>Move 0 files</button>
          <button className="btn btn-primary" disabled>Move selected</button>
        </div>
      </div>

      <EmptyState
        title="No files selected"
        description="Select files to move them to a destination folder."
        icon="move"
        buttonText="Select Files"
      />
    </div>
  );
}