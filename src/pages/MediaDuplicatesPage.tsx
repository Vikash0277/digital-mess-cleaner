import { EmptyState } from "../components/EmptyState";

export function MediaDuplicatesPage() {
  return (
    <div className="page">
      <h1 className="page-title">Video & Audio Duplicates</h1>
      <p className="page-subtitle">Find duplicate media files by content</p>

      <div className="media-duplicates-interface">
        <div className="media-tabs">
          <div className="tab active">Video Duplicates</div>
          <div className="tab">Audio Duplicates</div>
        </div>

        <div className="video-duplicate-groups">
          <div className="duplicate-group">
            <div className="group-header">
              <span>Similar videos (3)</span>
              <span>Potential savings: 2.4 GB</span>
            </div>
            <div className="group-files">
              <div className="file-row">
                <span>vacation_2023.mp4</span>
                <span>800 MB</span>
              </div>
              <div className="file-row">
                <span>vacation_2023_copy.mp4</span>
                <span>800 MB</span>
              </div>
              <div className="file-row">
                <span>vacation_final.mp4</span>
                <span>800 MB</span>
              </div>
            </div>
            <div className="group-actions">
              <button className="btn btn-caution">Move to Quarantine</button>
              <button className="btn btn-danger">Delete Permanently</button>
            </div>
          </div>

          <div className="duplicate-group">
            <div className="group-header">
              <span>Exact audio duplicates (5)</span>
              <span>Potential savings: 150 MB</span>
            </div>
            <div className="group-files">
              <div className="file-row">
                <span>song_favorite.mp3</span>
                <span>30 MB</span>
              </div>
              <div className="file-row">
                <span>song_favorite_copy1.mp3</span>
                <span>30 MB</span>
              </div>
              <div className="file-row">
                <span>song_favorite_copy2.mp3</span>
                <span>30 MB</span>
              </div>
              <div className="file-row">
                <span>song_favorite_copy3.mp3</span>
                <span>30 MB</span>
              </div>
              <div className="file-row">
                <span>song_favorite_copy4.mp3</span>
                <span>30 MB</span>
              </div>
            </div>
            <div className="group-actions">
              <button className="btn btn-caution">Move to Quarantine</button>
              <button className="btn btn-danger">Delete Permanently</button>
            </div>
          </div>
        </div>
      </div>

      <div className="scan-controls">
        <div className="scan-options">
          <label>
            <input type="checkbox" defaultChecked />
            <span>Include subfolders</span>
          </label>
          <label>
            <input type="checkbox" />
            <span>Compare by content hash</span>
          </label>
        </div>
        <button className="btn">Scan for Media Duplicates</button>
      </div>

      <EmptyState
        title="No media duplicates found"
        description="Scan your drives to detect duplicate videos and audio files."
        icon="film"
        buttonText="Start Media Scan"
      />
    </div>
  );
}