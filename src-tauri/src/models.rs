//! Serializable types shared between the Rust engine and the React UI.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Evidence {
    pub label: String,
    pub value: String,
    #[serde(default = "one")]
    pub weight: f64,
}

fn one() -> f64 {
    1.0
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    pub id: i64,
    pub root_id: i64,
    pub path: String,
    pub parent_path: String,
    pub name: String,
    pub ext: String,
    pub size: i64,
    pub mtime: i64,
    pub atime: i64,
    pub ctime: i64,
    pub category: String,
    pub mime: String,
    pub tags: Vec<String>,
    pub doc_type: Option<String>,
    pub confidence: f64,
    pub sensitive: bool,
    pub analyzed: bool,
    pub hash: Option<String>,
    #[serde(default)]
    pub snippet: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RootInfo {
    pub id: i64,
    pub path: String,
    pub label: String,
    pub kind: String,
    pub enabled: bool,
    pub last_scan_at: Option<i64>,
    pub file_count: i64,
    pub total_bytes: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryStat {
    pub category: String,
    pub bytes: i64,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionStat {
    pub ext: String,
    pub bytes: i64,
    pub count: i64,
}

/// On-demand drill-down for one category in "What takes up space".
/// Never computed on dashboard load: on a multi-million-row index the three
/// groupings are only worth their cost once the user asks for them.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryDetails {
    pub category: String,
    pub total_bytes: i64,
    pub file_count: i64,
    pub by_extension: Vec<ExtensionStat>,
    pub by_folder: Vec<FolderStat>,
    pub largest_files: Vec<FileEntry>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderStat {
    pub path: String,
    pub bytes: i64,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiskInfo {
    pub name: String,
    pub mount: String,
    pub total: i64,
    pub available: i64,
    pub kind: String,
    pub removable: bool,
}

/// One installed application. Sizes are measured from the local index when an
/// install folder is recorded, otherwise the installer's own estimate is used
/// and the UI says so. `evidence`/`confidence` explain the system/app split.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledApp {
    pub name: String,
    pub publisher: String,
    pub version: String,
    pub install_location: String,
    pub bytes: i64,
    pub file_count: i64,
    pub system: bool,
    pub size_source: String,
    pub evidence: String,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledApps {
    pub system_apps: Vec<InstalledApp>,
    pub user_apps: Vec<InstalledApp>,
    pub system_bytes: i64,
    pub user_bytes: i64,
    pub system_count: i64,
    pub user_count: i64,
    pub estimated_count: i64,
    pub platform: String,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub taken_at: i64,
    pub total_bytes: i64,
    pub file_count: i64,
    pub by_category: Vec<CategoryStat>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Dashboard {
    pub roots: Vec<RootInfo>,
    pub total_bytes: i64,
    pub file_count: i64,
    pub by_category: Vec<CategoryStat>,
    pub largest_folders: Vec<FolderStat>,
    pub duplicate_bytes: i64,
    pub duplicate_groups: i64,
    pub quarantine_count: i64,
    pub quarantine_bytes: i64,
    pub forgotten_count: i64,
    pub forgotten_bytes: i64,
    pub sensitive_count: i64,
    pub unanalyzed_count: i64,
    pub empty_folder_count: i64,
    pub reminders_due: i64,
    pub health_issues: i64,
    pub disks: Vec<DiskInfo>,
    pub last_scan_at: Option<i64>,
    pub prediction: Option<Prediction>,
    pub recent_activity: Vec<OperationEntry>,
    pub clutter: ClutterScore,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Page<T> {
    pub items: Vec<T>,
    pub total: i64,
    pub offset: i64,
    pub limit: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    pub file: FileEntry,
    pub snippet: String,
    pub matched_on: String,
    pub score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SearchFilters {
    #[serde(default)]
    pub categories: Vec<String>,
    #[serde(default)]
    pub exts: Vec<String>,
    #[serde(default)]
    pub doc_types: Vec<String>,
    #[serde(default)]
    pub root_id: Option<i64>,
    #[serde(default)]
    pub min_size: Option<i64>,
    #[serde(default)]
    pub max_size: Option<i64>,
    #[serde(default)]
    pub modified_after: Option<i64>,
    #[serde(default)]
    pub modified_before: Option<i64>,
    #[serde(default)]
    pub sensitive_only: bool,
    #[serde(default)]
    pub limit: Option<i64>,
    #[serde(default)]
    pub sort: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResponse {
    pub query: String,
    pub interpretation: Vec<String>,
    pub hits: Vec<SearchHit>,
    pub total: i64,
    pub took_ms: i64,
    pub mode: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AskAnswer {
    pub question: String,
    pub answer: String,
    pub references: Vec<SearchHit>,
    pub evidence: Vec<Evidence>,
    pub confidence: f64,
    pub caveat: Option<String>,
    pub took_ms: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateGroup {
    pub id: String,
    pub kind: String,
    pub signature: String,
    pub wasted_bytes: i64,
    pub similarity: Option<f64>,
    pub files: Vec<FileEntry>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateReport {
    pub mode: String,
    pub groups: Vec<DuplicateGroup>,
    pub total_wasted: i64,
    pub candidate_files: i64,
    pub hashed_files: i64,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuarantineEntry {
    pub id: String,
    pub original_path: String,
    pub quarantine_path: String,
    pub name: String,
    pub size: i64,
    pub created_at: i64,
    pub restored_at: Option<i64>,
    pub state: String,
    pub file_exists: bool,
    pub days_held: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuarantineResult {
    pub operation_id: String,
    pub moved: i64,
    pub failed: i64,
    pub total_bytes: i64,
    pub entries: Vec<QuarantineEntry>,
    pub outcomes: Vec<MoveOutcome>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationEntry {
    pub id: String,
    pub kind: String,
    pub summary: String,
    pub detail: String,
    pub reversible: bool,
    pub created_at: i64,
    pub undone_at: Option<i64>,
    pub item_count: i64,
    pub bytes: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrganizeRequestItem {
    pub file_id: i64,
    pub target_folder: String,
    pub new_name: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrganizeItem {
    pub file: FileEntry,
    pub suggested_name: String,
    pub suggested_folder: String,
    pub full_target: String,
    pub reason: String,
    pub confidence: f64,
    pub doc_type: Option<String>,
    pub evidence: Vec<Evidence>,
    pub conflict: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrganizePlan {
    pub folder: String,
    pub items: Vec<OrganizeItem>,
    pub total_bytes: i64,
    pub category_counts: Vec<CategoryStat>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveOutcome {
    pub file_id: i64,
    pub from: String,
    pub to: String,
    pub ok: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyResult {
    pub operation_id: String,
    pub moved: i64,
    pub failed: i64,
    pub outcomes: Vec<MoveOutcome>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Reminder {
    pub id: String,
    pub file_id: i64,
    pub title: String,
    pub due_date: String,
    pub kind: String,
    pub file_name: String,
    pub path: String,
    pub days_left: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentRecord {
    pub file: FileEntry,
    pub doc_type: String,
    pub vendor: Option<String>,
    pub amount: Option<String>,
    pub invoice_no: Option<String>,
    pub doc_date: Option<String>,
    pub due_date: Option<String>,
    pub confidence: f64,
    pub summary: String,
    pub evidence: Vec<Evidence>,
    pub related: Vec<FileEntry>,
    pub reminders: Vec<Reminder>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionGroup {
    pub key: String,
    pub latest_id: i64,
    pub reason: String,
    pub files: Vec<FileEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutomationRule {
    #[serde(default)]
    pub name_contains: Option<String>,
    #[serde(default)]
    pub extensions: Vec<String>,
    #[serde(default)]
    pub doc_types: Vec<String>,
    #[serde(default)]
    pub older_than_days: Option<i64>,
    #[serde(default)]
    pub min_size: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutomationActions {
    pub destination: String,
    #[serde(default)]
    pub rename_template: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Automation {
    pub id: String,
    pub name: String,
    pub root_id: i64,
    pub folder: String,
    pub enabled: bool,
    #[serde(default)]
    pub auto_apply: bool,
    #[serde(default)]
    pub watch_new: bool,
    pub rule: AutomationRule,
    pub actions: AutomationActions,
    pub created_at: i64,
    pub last_run_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutomationRun {
    pub id: String,
    pub automation_id: String,
    pub started_at: i64,
    pub finished_at: i64,
    pub actions: i64,
    pub summary: String,
    pub mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrganizerRule {
    pub id: String,
    pub name: String,
    #[serde(default = "enabled_default")]
    pub enabled: bool,
    #[serde(default)]
    pub folder: Option<String>,
    pub rule: AutomationRule,
    pub actions: AutomationActions,
}

fn enabled_default() -> bool {
    true
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthIssue {
    pub id: String,
    pub kind: String,
    pub severity: String,
    pub title: String,
    pub detail: String,
    pub path: Option<String>,
    pub bytes: i64,
    pub verified: bool,
    pub file_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Leftover {
    pub path: String,
    pub app: String,
    pub bytes: i64,
    pub last_modified: i64,
    pub confidence: f64,
    pub evidence: Vec<Evidence>,
    pub shared: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SensitiveFinding {
    pub file: FileEntry,
    pub kind: String,
    pub severity: String,
    pub excerpt: String,
    pub confidence: f64,
    pub evidence: Vec<Evidence>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Prediction {
    pub available_bytes: i64,
    pub growth_per_day: f64,
    pub days_until_full: Option<i64>,
    pub based_on_days: i64,
    pub message: String,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupSuggestion {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub detail: String,
    pub bytes: i64,
    pub confidence: f64,
    pub files: Vec<FileEntry>,
    pub evidence: Vec<Evidence>,
    pub safety: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClutterScore {
    pub score: i64,
    pub band: String,
    pub factors: Vec<Evidence>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MigrationSection {
    pub title: String,
    pub detail: String,
    pub bytes: i64,
    pub files: Vec<FileEntry>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MigrationChecklist {
    pub generated_at: i64,
    pub total_bytes: i64,
    pub file_count: i64,
    pub sections: Vec<MigrationSection>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineEvent {
    pub at: i64,
    pub kind: String,
    pub label: String,
    pub detail: String,
    pub file_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaBucket {
    pub key: String,
    pub count: i64,
    pub bytes: i64,
    pub files: Vec<FileEntry>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaOverview {
    pub image_count: i64,
    pub image_bytes: i64,
    pub screenshot_count: i64,
    pub screenshot_bytes: i64,
    pub video_count: i64,
    pub video_bytes: i64,
    pub audio_count: i64,
    pub audio_bytes: i64,
    pub screenshot_buckets: Vec<MediaBucket>,
    pub largest_videos: Vec<FileEntry>,
    pub largest_audio: Vec<FileEntry>,
    pub largest_images: Vec<FileEntry>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryDelta {
    pub category: String,
    pub from_bytes: i64,
    pub to_bytes: i64,
    pub delta_bytes: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageGrowth {
    pub since: i64,
    pub until: i64,
    pub categories: Vec<CategoryDelta>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Notification {
    pub id: String,
    pub title: String,
    pub body: String,
    pub kind: String,
    pub created_at: i64,
    pub read: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub app_version: String,
    pub data_dir: String,
    pub platform: String,
    pub db_bytes: i64,
    pub indexed_files: i64,
    pub root_count: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanProgress {
    pub scan_id: String,
    pub root_id: i64,
    pub phase: String,
    pub files_seen: i64,
    pub files_indexed: i64,
    pub files_skipped: i64,
    pub bytes_seen: i64,
    pub current_path: String,
    pub errors: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanReport {
    pub scan_id: String,
    pub files_seen: i64,
    pub files_indexed: i64,
    pub files_skipped: i64,
    pub files_removed: i64,
    pub bytes_seen: i64,
    pub errors: i64,
    pub cancelled: bool,
    pub took_ms: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyzeReport {
    pub analyzed: i64,
    pub failed: i64,
    pub cancelled: bool,
    pub took_ms: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudConnector {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub enabled: bool,
    pub synced_at: Option<i64>,
    pub file_count: i64,
    pub total_bytes: i64,
    pub status: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduledReport {
    pub id: String,
    pub name: String,
    pub schedule: String,
    pub format: String,
    pub last_run: Option<i64>,
    pub next_run: Option<i64>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportResult {
    pub report_id: String,
    pub generated_at: i64,
    pub format: String,
    pub bytes: i64,
    pub path: String,
    pub success: bool,
}
