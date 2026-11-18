import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { api } from "../lib/api";
import type { FileEntry } from "../lib/types";
import { FileInspectorProvider, useFileInspector } from "./FileInspector";

vi.mock("../lib/api", () => ({
  api: {
    fileDetails: vi.fn(),
    filePreview: vi.fn(),
    openFile: vi.fn(),
    revealFile: vi.fn(),
  },
  errorMessage: (error: unknown) => String(error),
}));

const file: FileEntry = {
  id: 7,
  rootId: 1,
  path: "C:\\Users\\Alex\\notes.txt",
  parentPath: "C:\\Users\\Alex",
  name: "notes.txt",
  ext: "txt",
  size: 20,
  mtime: 1_700_000_000,
  atime: 1_700_000_000,
  ctime: 1_700_000_000,
  category: "documents",
  mime: "text/plain",
  tags: [],
  docType: null,
  confidence: 0.9,
  sensitive: false,
  analyzed: false,
  hash: "abc123",
  snippet: null,
};

function Trigger() {
  const inspector = useFileInspector();
  return (
    <button type="button" onClick={() => inspector.openDetails(file)}>
      Inspect
    </button>
  );
}

describe("FileInspector", () => {
  it("shows indexed evidence and opens the file through the bridge", async () => {
    const user = userEvent.setup();
    vi.mocked(api.fileDetails).mockResolvedValue({
      file,
      exists: true,
      currentSize: 24,
      currentMtime: 1_700_000_100,
      rootLabel: "Documents",
      rootPath: "C:\\Users\\Alex",
    });
    vi.mocked(api.openFile).mockResolvedValue();

    render(
      <FileInspectorProvider>
        <Trigger />
      </FileInspectorProvider>,
    );

    await user.click(screen.getByRole("button", { name: "Inspect" }));

    expect(await screen.findByRole("dialog")).toBeInTheDocument();
    expect(await screen.findByText("C:\\Users\\Alex\\notes.txt")).toBeInTheDocument();
    expect(screen.getByText("Verified by full-content hash")).toBeInTheDocument();
    expect(screen.getByText("90%")).toBeInTheDocument();
    expect(screen.getByText("Chosen folder: Documents")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: /open with default app/i }));
    await waitFor(() => expect(api.openFile).toHaveBeenCalledWith(7));
  });
});
