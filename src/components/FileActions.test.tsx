import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { api } from "../lib/api";
import type { FileEntry } from "../lib/types";
import { FileActions } from "./FileActions";

vi.mock("../lib/api", () => ({
  api: {
    openFile: vi.fn(),
    revealFile: vi.fn(),
  },
  errorMessage: (error: unknown) => String(error),
}));

const file: FileEntry = {
  id: 12,
  rootId: 1,
  path: "C:\\Users\\Alex\\report.pdf",
  parentPath: "C:\\Users\\Alex",
  name: "report.pdf",
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
  analyzed: false,
  hash: null,
  snippet: null,
};

describe("FileActions", () => {
  it("opens and reveals only through the Rust bridge", async () => {
    const user = userEvent.setup();
    vi.mocked(api.openFile).mockResolvedValue();
    vi.mocked(api.revealFile).mockResolvedValue();

    render(<FileActions file={file} />);

    await user.click(screen.getByRole("button", { name: /open report.pdf/i }));
    await user.click(screen.getByRole("button", { name: /reveal report.pdf/i }));

    expect(api.openFile).toHaveBeenCalledWith(12);
    expect(api.revealFile).toHaveBeenCalledWith(12);
  });

  it("offers preview and details actions with plain-language labels", () => {
    render(<FileActions file={file} />);
    expect(
      screen.getByRole("button", { name: /preview report.pdf inside digital mess cleaner/i }),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /show details for report.pdf/i })).toBeInTheDocument();
  });
});
