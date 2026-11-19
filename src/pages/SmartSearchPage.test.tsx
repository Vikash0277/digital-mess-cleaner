import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { api } from "../lib/api";
import type { FileEntry, SearchResponse, TextIndexStatus } from "../lib/types";
import { SmartSearchPage } from "./SmartSearchPage";

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async () => () => {}),
}));

vi.mock("../lib/api", () => ({
  api: {
    searchFiles: vi.fn(),
    textIndexStatus: vi.fn(),
    ftsCheck: vi.fn(),
    textIndexStart: vi.fn(),
    textIndexCancel: vi.fn(),
    ftsRebuild: vi.fn(),
  },
  errorMessage: (error: unknown) => String(error),
}));

const file: FileEntry = {
  id: 1,
  rootId: 1,
  path: "C:\\Docs\\invoice.pdf",
  parentPath: "C:\\Docs",
  name: "invoice.pdf",
  ext: "pdf",
  size: 1024,
  mtime: 1_700_000_000,
  atime: 1_700_000_000,
  ctime: 1_700_000_000,
  category: "documents",
  mime: "application/pdf",
  tags: [],
  docType: null,
  confidence: 1,
  sensitive: false,
  analyzed: true,
  hash: null,
  snippet: "electricity invoice",
  snippetParts: [{ text: "electricity", highlight: true }],
};

const status: TextIndexStatus = {
  total: 10,
  indexed: 4,
  unsupported: 1,
  none: 5,
  missing: 0,
  ftsRows: 4,
  supportedRemaining: 5,
  lastIndexedAt: 0,
};

function response(total: number): SearchResponse {
  return {
    query: "electricity",
    interpretation: [],
    hits: [
      {
        file,
        snippet: file.snippet!,
        snippetParts: file.snippetParts!,
        matchedOn: "content",
        score: 1,
      },
    ],
    total,
    tookMs: 12,
    mode: "full-text",
  };
}

describe("SmartSearchPage", () => {
  beforeEach(() => {
    vi.mocked(api.textIndexStatus).mockResolvedValue(status);
    vi.mocked(api.ftsCheck).mockResolvedValue(true);
    vi.mocked(api.searchFiles).mockImplementation(async () => response(250));
  });

  it("pages through real full-text match counts", async () => {
    const user = userEvent.setup();
    render(<SmartSearchPage />);

    await user.type(screen.getByRole("textbox", { name: "Search your files" }), "electricity{enter}");

    await waitFor(() => expect(document.querySelector(".search-pager")).not.toBeNull());
    expect(document.querySelector(".search-pager")?.textContent).toContain("Showing 1–100 of 250");
    expect(document.querySelector(".search-summary-head")?.textContent).toContain("250");
    expect(api.searchFiles).toHaveBeenCalledWith(
      "electricity",
      expect.objectContaining({ limit: 100, offset: 0 }),
    );

    await user.click(screen.getByRole("button", { name: "Next" }));

    await waitFor(() =>
      expect(api.searchFiles).toHaveBeenCalledWith(
        "electricity",
        expect.objectContaining({ offset: 100 }),
      ),
    );
    await waitFor(() =>
      expect(document.querySelector(".search-pager")?.textContent).toContain(
        "Showing 101–200 of 250",
      ),
    );
  });

  it("offers local text indexing with plain-language copy", async () => {
    render(<SmartSearchPage />);
    expect(screen.getByRole("heading", { name: "Text index coverage" })).toBeInTheDocument();
    expect(screen.getByText(/Indexing never changes your files/)).toBeInTheDocument();
    expect(await screen.findByText(/4 files searchable/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Index text from my files" })).toBeEnabled();
  });
});
