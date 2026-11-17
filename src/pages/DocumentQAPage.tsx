import { EmptyState } from "../components/EmptyState";

export function DocumentQAPage() {
  return (
    <div className="page">
      <h1 className="page-title">Ask Document</h1>
      <p className="page-subtitle">Conversational document queries</p>

      <div className="qa-interface">
        <div className="question-input">
          <input
            type="text"
            placeholder="Ask about this document..."
            aria-label="Document question"
            onKeyDown={(e) => e.key === "Enter" && handleQuestion()}
          />
          <button className="btn btn-primary" onClick={handleQuestion}>
            Ask
          </button>
        </div>

        <div className="question-history">
          <h3>Recent Questions</h3>
          <ul>
            <li>What is this document about?</li>
            <li>Extract the key points</li>
            <li>Find the due date</li>
            <li>Who is the author?</li>
          </ul>
        </div>
      </div>

      <div className="answer-area">
        <h3>Answer</h3>
        <p>No question asked yet. Enter a question above to get started.</p>
      </div>

      <EmptyState
        title="No questions yet"
        description="Select a document to start asking questions."
        icon="message-circle"
      />
    </div>
  );
}

function handleQuestion() {
  // Trigger question - will be integrated with backend
}