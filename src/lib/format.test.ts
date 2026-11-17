import { categoryLabel, formatBytes, formatDate, pluralize } from "./format";

describe("formatBytes", () => {
  it("matches the Rust formatter for common sizes", () => {
    expect(formatBytes(0)).toBe("0 B");
    expect(formatBytes(1024)).toBe("1.0 KB");
    expect(formatBytes(1024 * 1024 * 1.5)).toBe("1.5 MB");
    expect(formatBytes(1024 * 1024 * 1024 * 2)).toBe("2.0 GB");
  });

  it("drops the decimal once a unit reaches three digits", () => {
    expect(formatBytes(150 * 1024)).toBe("150 KB");
    expect(formatBytes(100 * 1024 * 1024)).toBe("100 MB");
  });

  it("never shows a negative or invalid size", () => {
    expect(formatBytes(-5)).toBe("0 B");
    expect(formatBytes(Number.NaN)).toBe("0 B");
  });
});

describe("format helpers", () => {
  it("labels categories the same way as the backend", () => {
    expect(categoryLabel("documents")).toBe("Documents");
    expect(categoryLabel("video")).toBe("Video");
    expect(categoryLabel("unknown")).toBe("Other");
  });

  it("pluralises counts", () => {
    expect(pluralize(1, "file")).toBe("file");
    expect(pluralize(2, "file")).toBe("files");
    expect(pluralize(2, "category", "categories")).toBe("categories");
  });

  it("renders a dash for a missing date", () => {
    expect(formatDate(null)).toBe("—");
    expect(formatDate(0)).toBe("—");
  });
});
