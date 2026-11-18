import React from "react";
import {
  ArrowRightLeft,
  CalendarClock,
  ChartPie,
  Clock,
  Cloud,
  Copy,
  Download,
  FileText,
  Film,
  Folder,
  FolderTree,
  GitCompare,
  HeartPulse,
  Home,
  Images,
  LayoutGrid,
  Link2,
  MessageSquareText,
  MessagesSquare,
  Move,
  Pencil,
  Radar,
  ScrollText,
  Search,
  ShieldCheck,
  Table,
  Tags,
  TextSearch,
  Trash2,
  Usb,
  Workflow,
  Zap,
} from "lucide-react";
import { BrowserRouter as Router, Routes, Route } from "react-router-dom";
import { Sidebar, type NavGroup } from "./components/Sidebar";
import { Titlebar } from "./components/Titlebar";
import { DashboardPage } from "./pages/DashboardPage";
import { SmartSearchPage } from "./pages/SmartSearchPage";
import { FileInspectorProvider } from "./components/FileInspector";
import { MyFilesPage } from "./pages/MyFilesPage";
import { DuplicatesPage } from "./pages/DuplicatesPage";
import { MediaCleanupPage } from "./pages/MediaCleanupPage";
import { SmartOrganizerPage } from "./pages/SmartOrganizerPage";
import { DocumentIntelligencePage } from "./pages/DocumentIntelligencePage";
import { AskMyFilesPage } from "./pages/AskMyFilesPage";
import { CleanupCentrePage } from "./pages/CleanupCentrePage";
import { StorageInsightsPage } from "./pages/StorageInsightsPage";
import { ForgottenFilesPage } from "./pages/ForgottenFilesPage";
import { AutomationCentrePage } from "./pages/AutomationCentrePage";
import { PrivacyScannerPage } from "./pages/PrivacyScannerPage";
import { FileHealthPage } from "./pages/FileHealthPage";
import { MigrationAssistantPage } from "./pages/MigrationAssistantPage";
import { RulesEnginePage } from "./pages/RulesEnginePage";
import { DownloadsOrganiserPage } from "./pages/DownloadsOrganiserPage";
import { SearchPage } from "./pages/SearchPage";
import { CategorisationPage } from "./pages/CategorisationPage";
import { FolderSuggestionsPage } from "./pages/FolderSuggestionsPage";
import { BulkMovesPage } from "./pages/BulkMovesPage";
import { RenamingPage } from "./pages/RenamingPage";
import { DocumentQAPage } from "./pages/DocumentQAPage";
import { SummariesPage } from "./pages/SummariesPage";
import { ExtractionPage } from "./pages/ExtractionPage";
import { ComparisonPage } from "./pages/ComparisonPage";
import { SourceReferencesPage } from "./pages/SourceReferencesPage";
import { FolderMonitoringPage } from "./pages/FolderMonitoringPage";
import { ScheduledReportsPage } from "./pages/ScheduledReportsPage";
import { MediaDuplicatesPage } from "./pages/MediaDuplicatesPage";
import { ExternalDrivesPage } from "./pages/ExternalDrivesPage";
import { CloudConnectorsPage } from "./pages/CloudConnectorsPage";
import { ReviewProvider } from "./lib/review";
import { ScanProvider } from "./lib/scan";


const NAV_GROUPS: NavGroup[] = [
  {
    id: "library",
    label: "Library",
    items: [
      { key: "/", label: "Dashboard", Icon: <Home /> },
      { key: "/smart-search", label: "Smart Search", Icon: <Search /> },
      { key: "/my-files", label: "My Files", Icon: <Folder /> },
      { key: "/duplicates", label: "Duplicates", Icon: <Copy /> },
      { key: "/media-cleanup", label: "Media Cleanup", Icon: <Film /> },
      { key: "/forgotten-files", label: "Forgotten Files", Icon: <Clock /> },
    ],
  },
  {
    id: "cleanup",
    label: "Clean up",
    items: [
      { key: "/cleanup-centre", label: "Cleanup Centre", Icon: <Trash2 /> },
      { key: "/storage-insights", label: "Storage Insights", Icon: <ChartPie /> },
    ],
  },
  {
    id: "ai",
    label: "AI tools",
    items: [
      {
        key: "/smart-organizer",
        label: "Smart Organizer",
        Icon: <LayoutGrid />,
        badge: "Soon",
      },
      {
        key: "/document-intelligence",
        label: "Document Intelligence",
        Icon: <FileText />,
        badge: "Soon",
      },
      {
        key: "/ask-my-files",
        label: "Ask My Files",
        Icon: <MessagesSquare />,
        badge: "Soon",
      },
    ],
  },
  {
    id: "later",
    label: "Coming later",
    collapsible: true,
    items: [],
    subgroups: [
      {
        id: "more-tools",
        label: "More tools",
        items: [
          { key: "/automation-centre", label: "Automation Centre", Icon: <Zap /> },
          { key: "/privacy-scanner", label: "Privacy Scanner", Icon: <ShieldCheck /> },
          { key: "/file-health", label: "File Health", Icon: <HeartPulse /> },
          {
            key: "/migration-assistant",
            label: "Migration Assistant",
            Icon: <ArrowRightLeft />,
          },
        ],
      },
      {
        id: "release-2",
        label: "Release 2",
        items: [
          { key: "/rules-engine", label: "Rules Engine", Icon: <Workflow /> },
          { key: "/downloads-organiser", label: "Downloads Organiser", Icon: <Download /> },
          { key: "/search", label: "Advanced Search", Icon: <TextSearch /> },
          { key: "/categorisation", label: "Categorisation", Icon: <Tags /> },
          { key: "/folder-suggestions", label: "Folder Suggestions", Icon: <FolderTree /> },
          { key: "/bulk-moves", label: "Bulk Moves", Icon: <Move /> },
          { key: "/renaming", label: "Renaming", Icon: <Pencil /> },
        ],
      },
      {
        id: "release-3",
        label: "Release 3",
        items: [
          { key: "/document-qa", label: "Document Q&A", Icon: <MessageSquareText /> },
          { key: "/summaries", label: "Summaries", Icon: <ScrollText /> },
          { key: "/extraction", label: "Data Extraction", Icon: <Table /> },
          { key: "/comparison", label: "Comparison", Icon: <GitCompare /> },
          { key: "/source-references", label: "Source References", Icon: <Link2 /> },
        ],
      },
      {
        id: "release-4",
        label: "Release 4",
        items: [
          { key: "/folder-monitoring", label: "Folder Monitoring", Icon: <Radar /> },
          { key: "/scheduled-reports", label: "Scheduled Reports", Icon: <CalendarClock /> },
          {
            key: "/media-duplicates",
            label: "Video & Audio Duplicates",
            Icon: <Images />,
          },
          { key: "/external-drives", label: "External Drives", Icon: <Usb /> },
          { key: "/cloud-connectors", label: "Cloud Connectors", Icon: <Cloud /> },
        ],
      },
    ],
  },
];

function Shell() {
  return (
    <div className="app-shell">
      <a className="skip-link" href="#main-content">
        Skip to content
      </a>
      <Titlebar />
      <Sidebar groups={NAV_GROUPS} />
      <main className="main" id="main-content">
        <Routes>
          <Route path="/" element={<DashboardPage />} />
          <Route path="smart-search" element={<SmartSearchPage />} />
          <Route path="my-files" element={<MyFilesPage />} />
          <Route path="duplicates" element={<DuplicatesPage />} />
          <Route path="media-cleanup" element={<MediaCleanupPage />} />
          <Route path="smart-organizer" element={<SmartOrganizerPage />} />
          <Route path="document-intelligence" element={<DocumentIntelligencePage />} />
          <Route path="ask-my-files" element={<AskMyFilesPage />} />
          <Route path="cleanup-centre" element={<CleanupCentrePage />} />
          <Route path="storage-insights" element={<StorageInsightsPage />} />
          <Route path="forgotten-files" element={<ForgottenFilesPage />} />
          <Route path="automation-centre" element={<AutomationCentrePage />} />
          <Route path="rules-engine" element={<RulesEnginePage />} />
          <Route path="downloads-organiser" element={<DownloadsOrganiserPage />} />
          <Route path="search" element={<SearchPage />} />
          <Route path="categorisation" element={<CategorisationPage />} />
          <Route path="folder-suggestions" element={<FolderSuggestionsPage />} />
          <Route path="bulk-moves" element={<BulkMovesPage />} />
          <Route path="renaming" element={<RenamingPage />} />
          <Route path="document-qa" element={<DocumentQAPage />} />
          <Route path="summaries" element={<SummariesPage />} />
          <Route path="extraction" element={<ExtractionPage />} />
          <Route path="comparison" element={<ComparisonPage />} />
          <Route path="source-references" element={<SourceReferencesPage />} />
          <Route path="folder-monitoring" element={<FolderMonitoringPage />} />
          <Route path="scheduled-reports" element={<ScheduledReportsPage />} />
          <Route path="media-duplicates" element={<MediaDuplicatesPage />} />
          <Route path="external-drives" element={<ExternalDrivesPage />} />
          <Route path="cloud-connectors" element={<CloudConnectorsPage />} />
          <Route path="privacy-scanner" element={<PrivacyScannerPage />} />
          <Route path="file-health" element={<FileHealthPage />} />
          <Route path="migration-assistant" element={<MigrationAssistantPage />} />
        </Routes>
      </main>
    </div>
  );
}

export function App() {
  return (
    <React.StrictMode>
      <Router>
        <ReviewProvider>
          <ScanProvider>
            <FileInspectorProvider>
              <Shell />
            </FileInspectorProvider>
          </ScanProvider>
        </ReviewProvider>
      </Router>
    </React.StrictMode>
  );
}
