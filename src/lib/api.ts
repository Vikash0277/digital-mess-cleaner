// Typed bridge to the Rust commands registered in src-tauri/src/lib.rs.
// All filesystem work stays in Rust; this module only marshals arguments.

import { invoke } from "@tauri-apps/api/core";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import type {
  AppInfo,
  AskAnswer,
  CategoryDetails,
  CleanupSuggestion,
  Dashboard,
  DuplicateReport,
  FileDetails,
  FileEntry,
  FilePreview,
  FolderStat,
  InstalledApps,
  TextIndexStatus,
  MediaOverview,
  OperationEntry,
  Page,
  QuarantineEntry,
  QuarantineResult,
  RootInfo,
  ScanReport,
  SearchFilters,
  SearchResponse,
  Snapshot,
  StorageGrowth,
} from "./types";

export class BridgeError extends Error {
  code: string;

  constructor(code: string, message: string) {
    super(message);
    this.name = "BridgeError";
    this.code = code;
  }
}

/** True only inside the Tauri webview. Browser previews get a clear message. */
export function isDesktop(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

function toBridgeError(error: unknown): BridgeError {
  if (error instanceof BridgeError) return error;
  if (typeof error === "object" && error !== null) {
    const record = error as Record<string, unknown>;
    if (typeof record.message === "string") {
      return new BridgeError(
        typeof record.code === "string" ? record.code : "unknown_error",
        record.message,
      );
    }
  }
  return new BridgeError("unknown_error", String(error));
}

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (!isDesktop()) {
    throw new BridgeError(
      "not_desktop",
      "This action needs the Digital Mess Cleaner desktop app.",
    );
  }
  try {
    return await invoke<T>(command, args);
  } catch (error) {
    throw toBridgeError(error);
  }
}

export function errorMessage(error: unknown): string {
  if (error instanceof BridgeError) return error.message;
  if (error instanceof Error) return error.message;
  return String(error);
}

export const api = {
  appInfo: () => call<AppInfo>("app_info"),

  rootsList: () => call<RootInfo[]>("roots_list"),
  rootsAdd: (path: string, kind?: string) =>
    call<RootInfo[]>("roots_add", { path, kind: kind ?? null }),
  rootsRemove: (id: number) => call<RootInfo[]>("roots_remove", { id }),
  indexClear: (confirmation: string) => call<void>("index_clear", { confirmation }),

  dashboard: () => call<Dashboard>("dashboard"),
  snapshot: () => call<Snapshot>("storage_snapshot"),
  trends: (days?: number) => call<Snapshot[]>("storage_trends", { days: days ?? null }),
  growth: (days?: number) => call<StorageGrowth>("storage_growth", { days: days ?? null }),
  largestFiles: (limit?: number) =>
    call<FileEntry[]>("storage_largest_files", { limit: limit ?? null }),
  largestFolders: (limit?: number) =>
    call<FolderStat[]>("storage_largest_folders", { limit: limit ?? null }),
  categoryDetails: (category: string, limit?: number) =>
    call<CategoryDetails>("storage_category_details", {
      category,
      limit: limit ?? null,
    }),
  installedApps: () => call<InstalledApps>("installed_apps"),
  openFile: (fileId: number) => call<void>("file_open", { fileId }),
  revealFile: (fileId: number) => call<void>("file_reveal", { fileId }),
  fileDetails: (fileId: number) => call<FileDetails>("file_details", { fileId }),
  filePreview: (fileId: number) => call<FilePreview>("file_preview", { fileId }),
  forgottenFiles: (days?: number, minSize?: number, limit?: number) =>
    call<FileEntry[]>("storage_forgotten", {
      days: days ?? null,
      minSize: minSize ?? null,
      limit: limit ?? null,
    }),
  emptyFolders: () => call<FolderStat[]>("storage_empty_folders"),
  cleanupSuggestions: () => call<CleanupSuggestion[]>("storage_suggestions"),

  scanStart: (rootIds?: number[], maxFiles?: number) =>
    call<string>("scan_start", { rootIds: rootIds ?? null, maxFiles: maxFiles ?? null }),
  scanCancel: (scanId: string) => call<void>("scan_cancel", { scanId }),

  listFiles: (filters?: SearchFilters, limit?: number, offset?: number) =>
    call<Page<FileEntry>>("list_files", {
      filters: filters ?? {},
      limit: limit ?? null,
      offset: offset ?? null,
    }),
  searchFiles: (query: string, filters?: SearchFilters) =>
    call<SearchResponse>("search_files", { query, filters: filters ?? {} }),
  textIndexStatus: () => call<TextIndexStatus>("text_index_status"),
  textIndexStart: (limit?: number) => call<string>("text_index_start", { limit: limit ?? null }),
  textIndexCancel: (jobId: string) => call<void>("text_index_cancel", { jobId }),
  ftsCheck: () => call<boolean>("fts_check"),
  ftsRebuild: () => call<void>("fts_rebuild"),
  askFiles: (question: string) => call<AskAnswer>("ask_files", { question }),

  duplicatesAnalyze: (mode: string, threshold?: number) =>
    call<DuplicateReport>("duplicates_analyze", { mode, threshold: threshold ?? null }),

  mediaOverview: () => call<MediaOverview>("media_overview"),

  quarantineList: (includeRestored?: boolean) =>
    call<QuarantineEntry[]>("quarantine_list", {
      includeRestored: includeRestored ?? false,
    }),
  quarantineFiles: (fileIds: number[]) =>
    call<QuarantineResult>("quarantine_files", { fileIds }),
  quarantinePaths: (paths: string[], label?: string) =>
    call<QuarantineResult>("quarantine_paths", { paths, label: label ?? null }),
  quarantineRestore: (ids: string[]) =>
    call<QuarantineResult>("quarantine_restore", { ids }),
  quarantinePermanentDelete: (ids: string[], confirmation: string) =>
    call<QuarantineResult>("quarantine_permanent_delete", { ids, confirmation }),
  quarantinePurge: () => call<number>("quarantine_purge"),

  operationsList: (limit?: number) =>
    call<OperationEntry[]>("operations_list", { limit: limit ?? null }),
  operationUndo: (id: string) => call<OperationEntry>("operation_undo", { id }),
};

/**
 * Opens the OS folder picker. Returns null when the user cancels.
 * Only call this from a click handler inside the desktop app.
 */
export async function pickFolder(): Promise<string | null> {
  if (!isDesktop()) {
    throw new BridgeError("not_desktop", "Folder picking needs the desktop app.");
  }
  try {
    const selected = await openDialog({
      directory: true,
      multiple: false,
      title: "Choose a folder or drive to add",
    });
    return typeof selected === "string" ? selected : null;
  } catch (error) {
    throw toBridgeError(error);
  }
}

export type { ScanReport };
