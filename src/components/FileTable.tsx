import { useVirtualizer } from "@tanstack/react-virtual";
import { File, FileArchive, FileAudio, FileImage, FileText, FileVideo, Package } from "lucide-react";
import { useEffect, useRef, type ReactNode } from "react";
import { categoryLabel, formatBytes, formatDate } from "../lib/format";
import type { FileEntry } from "../lib/types";

const ICONS = {
  documents: FileText,
  images: FileImage,
  video: FileVideo,
  audio: FileAudio,
  archives: FileArchive,
  installers: Package,
  other: File,
} as const;

function CategoryIcon({ category }: { category: string }) {
  const Icon = ICONS[category as keyof typeof ICONS] ?? File;
  return (
    <span className="file-category-icon" data-category={category} aria-hidden="true">
      <Icon size={16} />
    </span>
  );
}

export interface FileTableProps {
  files: FileEntry[];
  selected?: ReadonlySet<number>;
  onToggle?: (id: number) => void;
  onToggleAll?: (checked: boolean) => void;
  caption?: string;
  empty?: ReactNode;
  /** Categories are hidden on the duplicates screen, where grouping carries it. */
  showCategory?: boolean;
}

export function FileTable({
  files,
  selected,
  onToggle,
  onToggleAll,
  caption = "Files",
  empty,
  showCategory = true,
}: FileTableProps) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const selectable = Boolean(selected && onToggle);
  const allChecked =
    selectable && files.length > 0 && files.every((file) => selected?.has(file.id));
  const someChecked = selectable && !allChecked && files.some((file) => selected?.has(file.id));
  const headerRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (headerRef.current) headerRef.current.indeterminate = someChecked;
  }, [someChecked]);

  const virtualizer = useVirtualizer({
    count: files.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => 48,
    overscan: 10,
  });

  if (files.length === 0) {
    return <div className="file-table-empty">{empty ?? "No files to show."}</div>;
  }

  return (
    <div
      className="file-table"
      role="grid"
      aria-label={caption}
      aria-rowcount={files.length + 1}
      data-selectable={selectable || undefined}
      data-category={showCategory || undefined}
      tabIndex={0}
    >
      <div className="file-table-head" role="row">
        {selectable && (
          <span className="file-table-check" role="columnheader" aria-label="Selection">
            <input
              ref={headerRef}
              type="checkbox"
              checked={allChecked}
              aria-label="Select all files in this list"
              onChange={(event) => onToggleAll?.(event.target.checked)}
            />
          </span>
        )}
        <span role="columnheader" className="file-table-name">
          Name
        </span>
        <span role="columnheader" className="file-table-path">
          Location
        </span>
        <span role="columnheader" className="file-table-size">
          Size
        </span>
        <span role="columnheader" className="file-table-date">
          Modified
        </span>
        {showCategory && (
          <span role="columnheader" className="file-table-category">
            Category
          </span>
        )}
      </div>

      <div ref={scrollRef} className="file-table-body" aria-label={`${caption} list`}>
        <div style={{ height: virtualizer.getTotalSize(), position: "relative" }}>
          {virtualizer.getVirtualItems().map((item) => {
            const file = files[item.index];
            if (!file) return null;
            const isSelected = selected?.has(file.id) ?? false;
            return (
              <div
                key={file.id}
                role="row"
                aria-rowindex={item.index + 2}
                className="file-table-row"
                data-selected={isSelected || undefined}
                style={{
                  position: "absolute",
                  top: 0,
                  left: 0,
                  width: "100%",
                  height: item.size,
                  transform: `translateY(${item.start}px)`,
                }}
              >
                {selectable && (
                  <span className="file-table-check" role="gridcell">
                    <input
                      type="checkbox"
                      checked={isSelected}
                      aria-label={`Select ${file.name}`}
                      onChange={() => onToggle?.(file.id)}
                    />
                  </span>
                )}
                <span className="file-table-name" role="gridcell" title={file.path}>
                  <CategoryIcon category={file.category} />
                  <span className="file-table-name-text">
                    <span className="file-table-file-name">{file.name}</span>
                    {file.snippet && file.snippet !== file.path && (
                      <span className="file-table-snippet">{file.snippet}</span>
                    )}
                  </span>
                </span>
                <span className="file-table-path" role="gridcell" title={file.parentPath}>
                  {file.parentPath}
                </span>
                <span className="file-table-size" role="gridcell">
                  {formatBytes(file.size)}
                </span>
                <span className="file-table-date" role="gridcell">
                  {formatDate(file.mtime)}
                </span>
                {showCategory && (
                  <span className="file-table-category" role="gridcell">
                    <span className="badge">{categoryLabel(file.category)}</span>
                  </span>
                )}
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}
