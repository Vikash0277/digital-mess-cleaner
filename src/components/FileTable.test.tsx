import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { FileTable } from "./FileTable";
import type { FileEntry } from "../lib/types";

// jsdom has no layout, so the real virtualizer would render zero rows.
vi.mock("@tanstack/react-virtual", () => ({
  useVirtualizer: ({ count }: { count: number }) => ({
    getTotalSize: () => count * 48,
    getVirtualItems: () =>
      Array.from({ length: count }, (_, index) => ({
        index,
        start: index * 48,
        size: 48,
      })),
  }),
}));

function makeFile(id: number, name: string, size: number): FileEntry {
  return {
    id,
    rootId: 1,
    path: `C:\\Users\\Alex\\Documents\\${name}`,
    parentPath: "C:\\Users\\Alex\\Documents",
    name,
    ext: "pdf",
    size,
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
}

const files = [
  makeFile(1, "Q4 Financial Report.pdf", 8 * 1024 * 1024),
  makeFile(2, "Notes.pdf", 2048),
];

describe("FileTable", () => {
  it("renders file details without a selection column when read-only", () => {
    render(<FileTable files={files} caption="Indexed files" />);

    expect(screen.getByRole("grid", { name: "Indexed files" })).toBeInTheDocument();
    expect(screen.getByText("Q4 Financial Report.pdf")).toBeInTheDocument();
    expect(screen.getByText("8.0 MB")).toBeInTheDocument();
    expect(screen.queryAllByRole("checkbox")).toHaveLength(0);
  });

  it("toggles a single file through its checkbox", async () => {
    const user = userEvent.setup();
    const onToggle = vi.fn();

    render(
      <FileTable
        files={files}
        selected={new Set()}
        onToggle={onToggle}
        onToggleAll={() => {}}
        caption="Indexed files"
      />,
    );

    await user.click(screen.getByRole("checkbox", { name: "Select Notes.pdf" }));
    expect(onToggle).toHaveBeenCalledWith(2);
  });

  it("marks the header checkbox as indeterminate for a partial selection", () => {
    render(
      <FileTable
        files={files}
        selected={new Set([1])}
        onToggle={() => {}}
        onToggleAll={() => {}}
        caption="Indexed files"
      />,
    );

    const header = screen.getByRole("checkbox", {
      name: "Select all files in this list",
    }) as HTMLInputElement;
    expect(header.indeterminate).toBe(true);
    expect(header.checked).toBe(false);
  });

  it("selects every listed file from the header", async () => {
    const user = userEvent.setup();
    const onToggleAll = vi.fn();

    render(
      <FileTable
        files={files}
        selected={new Set()}
        onToggle={() => {}}
        onToggleAll={onToggleAll}
        caption="Indexed files"
      />,
    );

    const header = screen.getByRole("checkbox", {
      name: "Select all files in this list",
    });
    expect(header).not.toBeChecked();

    await user.click(header);
    expect(onToggleAll).toHaveBeenCalledWith(true);
  });

  it("clears the selection from the header when everything is selected", async () => {
    const user = userEvent.setup();
    const onToggleAll = vi.fn();

    render(
      <FileTable
        files={files}
        selected={new Set([1, 2])}
        onToggle={() => {}}
        onToggleAll={onToggleAll}
        caption="Indexed files"
      />,
    );

    const header = screen.getByRole("checkbox", {
      name: "Select all files in this list",
    });
    expect(header).toBeChecked();

    await user.click(header);
    expect(onToggleAll).toHaveBeenCalledWith(false);
  });

  it("shows the empty slot instead of an empty table", () => {
    render(<FileTable files={[]} empty="Nothing indexed yet." />);

    expect(screen.getByText("Nothing indexed yet.")).toBeInTheDocument();
    expect(screen.queryByRole("grid")).not.toBeInTheDocument();
  });
});
