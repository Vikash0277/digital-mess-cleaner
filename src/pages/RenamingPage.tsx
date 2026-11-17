import { EmptyState } from "../components/EmptyState";

export function RenamingPage() {
  return (
    <div className="page">
      <h1 className="page-title">Renaming</h1>
      <p className="page-subtitle">Batch rename your files</p>

      <div className="renaming-interface">
        <div className="file-list">
          <div className="file-item">
            <input type="text" readOnly value="document.pdf" />
            <span className="original-name">document.pdf</span>
          </div>
          <div className="file-item">
            <input type="text" readOnly value="image.png" />
            <span className="original-name">image.png</span>
          </div>
          <div className="file-item">
            <input type="text" readOnly value="presentation.pptx" />
            <span className="original-name">presentation.pptx</span>
          </div>
        </div>

        <div className="renaming-pattern">
          <h3>Renaming Pattern</h3>
          <div className="pattern-options">
            <div className="pattern-option">
              <input type="radio" name="pattern" id="pattern-sequential" />
              <label htmlFor="pattern-sequential">
                Sequential: file_001, file_002, file_003
              </label>
            </div>
            <div className="pattern-option">
              <input type="radio" name="pattern" id="pattern-date" />
              <label htmlFor="pattern-date">
                Date-based: file_20240115_001
              </label>
            </div>
            <div className="pattern-option">
              <input type="radio" name="pattern" id="pattern-custom" defaultChecked />
              <label htmlFor="pattern-custom">
                Custom: myfiles_ (user defined)
              </label>
            </div>
          </div>
        </div>

        <div className="pattern-preview">
          <h3>Preview</h3>
          <div className="preview-list">
            <div className="preview-item">
              <span>document.pdf → myfiles_001.pdf</span>
            </div>
            <div className="preview-item">
              <span>image.png → myfiles_002.png</span>
            </div>
            <div className="preview-item">
              <span>presentation.pptx → myfiles_003.pptx</span>
            </div>
          </div>
        </div>
      </div>

      <div className="action-controls">
        <button className="btn" disabled>Rename 0 files</button>
        <button className="btn btn-primary" disabled>Rename</button>
      </div>

      <EmptyState
        title="No files selected"
        description="Select files to rename them with a pattern."
        icon="rename"
        buttonText="Select Files"
      />
    </div>
  );
}