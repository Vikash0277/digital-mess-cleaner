import { EmptyState } from "../components/EmptyState";

export function AskMyFilesPage() {
  return (
    <div className="page">
      <h1 className="page-title">Ask My Files</h1>
      <p className="page-subtitle">Conversational file search</p>

      <div className="ask-interface">
        <div className="input-group">
          <input
            type="text"
            placeholder="Ask about your files..."
            aria-label="Search files"
          />
          <button className="btn btn-primary">Ask</button>
        </div>

        <div className="recent-queries">
          <h3>Recent queries</h3>
          <ul>
            <li>Find my laptop invoice</li>
            <li>Show me PDFs from last year</li>
            <li>Which folders have old project files</li>
            <li>Files I haven't opened in two years</li>
          </ul>
        </div>
      </div>

      <EmptyState
        title="Start asking"
        description="Type a question about your files to get started."
        icon="message-circle"
      />
    </div>
  );
}