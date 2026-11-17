import { Image, Repeat } from "lucide-react";
import { Link, useNavigate } from "react-router-dom";
import { EmptyState } from "../components/EmptyState";
import { FileTable } from "../components/FileTable";
import { ErrorNotice, LoadingNotice } from "../components/Notice";
import { StatCard } from "../components/StatCard";
import { api } from "../lib/api";
import { formatBytes, formatCount, pluralize } from "../lib/format";
import { useAsync } from "../lib/hooks";

const loadMedia = () => api.mediaOverview();

export function MediaCleanupPage() {
  const media = useAsync(loadMedia);
  const navigate = useNavigate();
  const data = media.data;

  return (
    <div className="page">
      <div className="page-heading">
        <div>
          <h1 className="page-title">Media Cleanup</h1>
          <p className="page-subtitle">Photos, screenshots, videos and audio at a glance</p>
        </div>
        <div className="page-heading-actions">
          <Link className="btn" to="/duplicates">
            <Repeat size={14} aria-hidden="true" />
            Find similar images
          </Link>
        </div>
      </div>

      {media.loading && !data && <LoadingNotice label="Reading media files from your index…" />}

      {media.error && (
        <ErrorNotice
          message={media.error}
          onRetry={media.reload}
          desktopOnly={media.errorCode === "not_desktop"}
        />
      )}

      {data && (
        <>
          <div className="stats-grid">
            <StatCard
              title="Screenshots"
              value={formatCount(data.screenshotCount)}
              subtitleOf={formatBytes(data.screenshotBytes)}
            />
            <StatCard
              title="Images"
              value={formatCount(data.imageCount)}
              subtitleOf={formatBytes(data.imageBytes)}
            />
            <StatCard
              title="Videos"
              value={formatCount(data.videoCount)}
              subtitleOf={formatBytes(data.videoBytes)}
            />
            <StatCard
              title="Audio"
              value={formatCount(data.audioCount)}
              subtitleOf={formatBytes(data.audioBytes)}
            />
          </div>

          {data.imageCount + data.videoCount + data.audioCount === 0 && (
            <EmptyState
              title="No media indexed yet"
              description="Add a folder on the Dashboard and run a scan. Photos and videos are only read from folders you choose."
              icon="image"
              buttonText="Go to Dashboard"
              onAction={() => navigate("/")}
            />
          )}

          {data.screenshotBuckets.length > 0 && (
            <section className="card" aria-labelledby="screenshot-buckets-title">
              <div className="section-head">
                <h2 id="screenshot-buckets-title" className="section-title">
                  <Image size={16} aria-hidden="true" /> Screenshot groups
                </h2>
                <span className="muted">
                  {formatCount(data.screenshotCount)}{" "}
                  {pluralize(data.screenshotCount, "screenshot")} ·{" "}
                  {formatBytes(data.screenshotBytes)}
                </span>
              </div>

              <ul className="location-list">
                {data.screenshotBuckets.map((bucket) => (
                  <li key={bucket.key} className="location-item">
                    <div className="location-info">
                      <span className="location-label">{bucket.key}</span>
                      <span className="location-meta">
                        {formatCount(bucket.count)} {pluralize(bucket.count, "file")}
                      </span>
                    </div>
                    <span className="muted">{formatBytes(bucket.bytes)}</span>
                  </li>
                ))}
              </ul>
            </section>
          )}

          {data.largestImages.length > 0 && (
            <section className="card" aria-labelledby="largest-images-title">
              <h2 id="largest-images-title" className="section-title">
                Largest images
              </h2>
              <div className="media-table">
                <FileTable files={data.largestImages} caption="Largest images" showCategory={false} />
              </div>
            </section>
          )}

          {data.largestVideos.length > 0 && (
            <section className="card" aria-labelledby="largest-videos-title">
              <h2 id="largest-videos-title" className="section-title">
                Largest videos
              </h2>
              <div className="media-table">
                <FileTable files={data.largestVideos} caption="Largest videos" showCategory={false} />
              </div>
            </section>
          )}

          {data.largestAudio.length > 0 && (
            <section className="card" aria-labelledby="largest-audio-title">
              <h2 id="largest-audio-title" className="section-title">
                Largest audio files
              </h2>
              <div className="media-table">
                <FileTable files={data.largestAudio} caption="Largest audio files" showCategory={false} />
              </div>
            </section>
          )}

          {data.notes.length > 0 && (
            <section className="card" aria-label="How media is grouped">
              <h2 className="section-title">Evidence used</h2>
              <ul className="notes-list">
                {data.notes.map((note) => (
                  <li key={note}>{note}</li>
                ))}
              </ul>
            </section>
          )}
        </>
      )}
    </div>
  );
}
