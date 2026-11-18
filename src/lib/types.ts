// TypeScript mirrors of the Rust models in src-tauri/src/models.rs.
// Every struct there is serialised with camelCase field names.

export interface Evidence {
  label: string;
  value: string;
  weight: number;
}

export interface AppInfo {
  appVersion: string;
  dataDir: string;
  platform: string;
  dbBytes: number;
  indexedFiles: number;
  rootCount: number;
}

export interface RootInfo {
  id: number;
  path: string;
  label: string;
  kind: string;
  enabled: boolean;
  lastScanAt: number | null;
  fileCount: number;
  totalBytes: number;
}

export interface FileEntry {
  id: number;
  rootId: number;
  path: string;
  parentPath: string;
  name: string;
  ext: string;
  size: number;
  mtime: number;
  atime: number;
  ctime: number;
  category: string;
  mime: string;
  tags: string[];
  docType: string | null;
  confidence: number;
  sensitive: boolean;
  analyzed: boolean;
  hash: string | null;
  snippet: string | null;
}

export interface FileDetails {
  file: FileEntry;
  exists: boolean;
  currentSize: number;
  currentMtime: number;
  rootLabel: string;
  rootPath: string;
}

export interface FilePreview {
  kind: "text" | "image" | "pdf" | "unsupported" | "too_large";
  mime: string;
  text: string | null;
  data: string | null;
  reason: string;
  truncated: boolean;
  limitBytes: number;
}

export interface CategoryStat {
  category: string;
  bytes: number;
  count: number;
}

export interface ExtensionStat {
  ext: string;
  bytes: number;
  count: number;
}

export interface CategoryDetails {
  category: string;
  totalBytes: number;
  fileCount: number;
  byExtension: ExtensionStat[];
  byFolder: FolderStat[];
  largestFiles: FileEntry[];
}

export interface InstalledApp {
  name: string;
  publisher: string;
  version: string;
  installLocation: string;
  bytes: number;
  fileCount: number;
  system: boolean;
  sizeSource: string;
  evidence: string;
  confidence: number;
}

export interface InstalledApps {
  systemApps: InstalledApp[];
  userApps: InstalledApp[];
  systemBytes: number;
  userBytes: number;
  systemCount: number;
  userCount: number;
  estimatedCount: number;
  platform: string;
  notes: string[];
}

export interface FolderStat {
  path: string;
  bytes: number;
  count: number;
}

export interface DiskInfo {
  name: string;
  mount: string;
  total: number;
  available: number;
  kind: string;
  removable: boolean;
}

export interface Prediction {
  availableBytes: number;
  growthPerDay: number;
  daysUntilFull: number | null;
  basedOnDays: number;
  message: string;
  confidence: number;
}

export interface ClutterScore {
  score: number;
  band: string;
  factors: Evidence[];
}

export interface OperationEntry {
  id: string;
  kind: string;
  summary: string;
  detail: string;
  reversible: boolean;
  createdAt: number;
  undoneAt: number | null;
  itemCount: number;
  bytes: number;
}

export interface Snapshot {
  takenAt: number;
  totalBytes: number;
  fileCount: number;
  byCategory: CategoryStat[];
}

export interface StorageGrowth {
  since: number;
  until: number;
  categories: {
    category: string;
    fromBytes: number;
    toBytes: number;
    deltaBytes: number;
  }[];
  notes: string[];
}

export interface Dashboard {
  roots: RootInfo[];
  totalBytes: number;
  fileCount: number;
  byCategory: CategoryStat[];
  largestFolders: FolderStat[];
  duplicateBytes: number;
  duplicateGroups: number;
  quarantineCount: number;
  quarantineBytes: number;
  forgottenCount: number;
  forgottenBytes: number;
  sensitiveCount: number;
  unanalyzedCount: number;
  emptyFolderCount: number;
  remindersDue: number;
  healthIssues: number;
  disks: DiskInfo[];
  lastScanAt: number | null;
  prediction: Prediction | null;
  recentActivity: OperationEntry[];
  clutter: ClutterScore;
}

export interface Page<T> {
  items: T[];
  total: number;
  offset: number;
  limit: number;
}

export interface SearchFilters {
  categories?: string[];
  exts?: string[];
  docTypes?: string[];
  rootId?: number | null;
  minSize?: number | null;
  maxSize?: number | null;
  modifiedAfter?: number | null;
  modifiedBefore?: number | null;
  sensitiveOnly?: boolean;
  limit?: number | null;
  sort?: string | null;
}

export interface SearchHit {
  file: FileEntry;
  snippet: string;
  matchedOn: string;
  score: number;
}

export interface SearchResponse {
  query: string;
  interpretation: string[];
  hits: SearchHit[];
  total: number;
  tookMs: number;
  mode: string;
}

export interface DuplicateGroup {
  id: string;
  kind: string;
  signature: string;
  wastedBytes: number;
  similarity: number | null;
  files: FileEntry[];
}

export interface DuplicateReport {
  mode: string;
  groups: DuplicateGroup[];
  totalWasted: number;
  candidateFiles: number;
  hashedFiles: number;
  notes: string[];
}

export interface QuarantineEntry {
  id: string;
  originalPath: string;
  quarantinePath: string;
  name: string;
  size: number;
  createdAt: number;
  restoredAt: number | null;
  state: string;
  fileExists: boolean;
  daysHeld: number;
}

export interface MoveOutcome {
  fileId: number;
  from: string;
  to: string;
  ok: boolean;
  error: string | null;
}

export interface QuarantineResult {
  operationId: string;
  moved: number;
  failed: number;
  totalBytes: number;
  entries: QuarantineEntry[];
  outcomes: MoveOutcome[];
}

export interface MediaBucket {
  key: string;
  count: number;
  bytes: number;
  files: FileEntry[];
}

export interface MediaOverview {
  imageCount: number;
  imageBytes: number;
  screenshotCount: number;
  screenshotBytes: number;
  videoCount: number;
  videoBytes: number;
  audioCount: number;
  audioBytes: number;
  screenshotBuckets: MediaBucket[];
  largestVideos: FileEntry[];
  largestAudio: FileEntry[];
  largestImages: FileEntry[];
  notes: string[];
}

export interface CleanupSuggestion {
  id: string;
  kind: string;
  title: string;
  detail: string;
  bytes: number;
  confidence: number;
  files: FileEntry[];
  evidence: Evidence[];
  safety: string;
}

export interface ScanProgress {
  scanId: string;
  rootId: number;
  phase: string;
  filesSeen: number;
  filesIndexed: number;
  filesSkipped: number;
  bytesSeen: number;
  currentPath: string;
  errors: number;
}

export interface ScanReport {
  scanId: string;
  filesSeen: number;
  filesIndexed: number;
  filesSkipped: number;
  filesRemoved: number;
  bytesSeen: number;
  errors: number;
  cancelled: boolean;
  tookMs: number;
}

export interface ScanDone {
  scanId: string;
  reports: ScanReport[];
  errors: string[];
  cancelled: boolean;
}

export interface AskAnswer {
  question: string;
  answer: string;
  references: SearchHit[];
  evidence: Evidence[];
  confidence: number;
  caveat: string | null;
  tookMs: number;
}
