import { useEffect, useState } from "react";
import { panelClass, primaryButtonClass, SEARCH_PLACEHOLDERS, SUGGESTIONS } from "../app/constants";
import type { FileDetails, IndexedRoot, SavedResult, ViewName } from "../app/types";
import { SelectedFilePreview } from "../components/preview";
import { DotsIcon, PinIcon, SparkleIcon, iconForKind, BookmarkIcon } from "../components/icons";
import { OverviewCard } from "../components/cards";
import { cx, kindLabel, shortPath, statusLabel, syncStatusLabel, formatRelativeDate } from "../lib/appHelpers";

export interface HomeViewProps {
  roots: IndexedRoot[];
  totalFiles: number;
  runningIndexCount: number;
  query: string;
  setQuery: (value: string) => void;
  setCurrentView: (view: ViewName) => void;
  recentSearches: string[];
  savedResults: SavedResult[];
  selectedFile: FileDetails | null;
  selectedPreviewUrl: string | null;
  pinnedRoots: IndexedRoot[];
  bindPreviewNode: (node: HTMLDivElement | null) => void;
  onShowSavedResult: (path: string) => Promise<void>;
  onRemoveSavedResult: (path: string) => void;
  message: string | null;
}

export function HomeView({
  roots,
  totalFiles,
  runningIndexCount,
  query,
  setQuery,
  setCurrentView,
  recentSearches,
  savedResults,
  selectedFile,
  selectedPreviewUrl,
  pinnedRoots,
  bindPreviewNode,
  onShowSavedResult,
  onRemoveSavedResult,
  message,
}: HomeViewProps) {
  const [placeholderIdx, setPlaceholderIdx] = useState(0);
  const [placeholderVisible, setPlaceholderVisible] = useState(true);

  useEffect(() => {
    const id = setInterval(() => {
      setPlaceholderVisible(false);
      setTimeout(() => {
        setPlaceholderIdx((i) => (i + 1) % SEARCH_PLACEHOLDERS.length);
        setPlaceholderVisible(true);
      }, 350);
    }, 3000);
    return () => clearInterval(id);
  }, []);

  return (
    <div className="space-y-6">
      <section className="px-1 pt-2 text-center">
        <p className="text-[0.82rem] uppercase tracking-[0.22em] text-[#727792]">
          Workspace intelligence
        </p>
        <h1 className="display-type mx-auto mt-4 max-w-4xl text-[clamp(2.8rem,6vw,4.5rem)] leading-[0.95] text-[#242b28]">
          Search your files in plain English.
        </h1>
        <p className="mx-auto mt-4 max-w-2xl text-[1.1rem] leading-8 text-[#6b726e]">
          Mira turns folders into a searchable workspace so you can find documents,
          images, and text-heavy files — without remembering exact filenames.
        </p>

        <div className="mx-auto mt-8 max-w-5xl rounded-[28px] border border-black/5 bg-white/78 p-3 shadow-[0_22px_60px_rgba(85,93,122,0.08)]">
          <div className="flex flex-col gap-3 lg:flex-row">
            <div className="flex flex-1 items-center gap-4 rounded-[22px] bg-[#fcfbf8] px-5 py-4">
              <SparkleIcon className="h-6 w-6 shrink-0 text-[#737792]" />
              <div className="relative min-w-0 flex-1">
                <input
                  className="w-full bg-transparent text-[1.15rem] text-[#222825] outline-none"
                  value={query}
                  onChange={(event) => setQuery(event.target.value)}
                  autoComplete="off"
                  autoCorrect="off"
                  autoCapitalize="none"
                  spellCheck={false}
                  placeholder=""
                />
                {query === "" && (
                  <span
                    className="pointer-events-none absolute inset-y-0 left-0 flex items-center text-[1.15rem] text-[#9da2a6] transition-opacity duration-300"
                    style={{ opacity: placeholderVisible ? 1 : 0 }}
                  >
                    {SEARCH_PLACEHOLDERS[placeholderIdx]}
                  </span>
                )}
              </div>
            </div>
            <button
              className={cx(primaryButtonClass, "min-w-[180px] rounded-[22px] px-6")}
              onClick={() => setCurrentView("results")}
            >
              Analyze
            </button>
          </div>
        </div>

        <div className="mt-7">
          <p className="text-[0.75rem] uppercase tracking-[0.22em] text-[#8a8f93]">
            Suggested
          </p>
          <div className="mt-4 flex flex-wrap items-center justify-center gap-3">
            {SUGGESTIONS.map((suggestion) => (
              <button
                key={suggestion}
                className="rounded-full bg-[#eef0ed] px-5 py-2.5 text-sm font-medium text-[#313735] transition hover:bg-[#e5e7e4]"
                onClick={() => {
                  setQuery(suggestion);
                  setCurrentView("results");
                }}
              >
                {suggestion}
              </button>
            ))}
          </div>
        </div>

        {recentSearches.length > 0 ? (
          <div className="mt-6">
            <p className="text-[0.75rem] uppercase tracking-[0.22em] text-[#8a8f93]">
              Recent searches
            </p>
            <div className="mt-4 flex flex-wrap items-center justify-center gap-3">
              {recentSearches.map((recent) => (
                <button
                  key={recent}
                  className="rounded-full border border-black/5 bg-white px-4 py-2.5 text-sm font-medium text-[#4f5652] transition hover:-translate-y-0.5 hover:bg-[#f8f6f1]"
                  onClick={() => {
                    setQuery(recent);
                    setCurrentView("results");
                  }}
                >
                  {recent}
                </button>
              ))}
            </div>
          </div>
        ) : null}
      </section>

      <section className="grid gap-4 xl:grid-cols-[1.45fr_0.75fr]">
        <article className={cx(panelClass, "p-6")}>
          <div className="flex items-start justify-between gap-4">
            <div>
              <span className="rounded-full bg-[#e4e7fa] px-3 py-1 text-[0.78rem] uppercase tracking-[0.12em] text-[#58607e]">
                Workspace insight
              </span>
              <h2 className="display-type mt-4 text-[2rem] leading-tight text-[#202724]">
                Search-ready file universe
              </h2>
            </div>
            <button className="grid h-10 w-10 place-items-center rounded-full bg-[#f5f4f1] text-[#7c8187]">
              <DotsIcon />
            </button>
          </div>

          <div className="mt-6 grid gap-4 lg:grid-cols-[1.1fr_0.9fr]">
            <div className="rounded-[24px] bg-[#f9f8f5] p-5">
              <div className="grid h-[250px] grid-cols-7 items-end gap-2 rounded-[20px] border border-dashed border-[#e5e2da] bg-[radial-gradient(circle_at_center,rgba(228,231,250,0.22),transparent_60%)] p-4">
                {[28, 36, 58, 76, 102, 72, 46].map((value, index) => (
                  <div
                    key={value}
                    className={cx(
                      "rounded-t-[18px] bg-[#d9dbe5]",
                      index === 4 && "bg-[#737792]",
                    )}
                    style={{ height: `${value}%` }}
                  />
                ))}
              </div>
            </div>

            <div className="grid gap-4">
              <OverviewCard
                label="Files indexed"
                value={totalFiles.toLocaleString()}
                meta={`${roots.length} connected source${roots.length === 1 ? "" : "s"}`}
              />
              <OverviewCard
                label="Search coverage"
                value={roots.length === 0 ? "0%" : `${Math.min(99, 44 + roots.length * 9)}%`}
                meta="Filename + extracted text + local CLIP embeddings"
              />
              <OverviewCard
                label="Indexer status"
                value={runningIndexCount > 0 ? "Active" : "Ready"}
                meta={
                  runningIndexCount > 0
                    ? `${runningIndexCount} source${runningIndexCount === 1 ? "" : "s"} in progress`
                    : "Waiting for your next search"
                }
              />
            </div>
          </div>
        </article>

        <aside className={cx(panelClass, "p-6")}>
          <div className="flex items-center justify-between gap-3">
            <h3 className="display-type text-[1.7rem] text-[#202724]">Pinned sources</h3>
            <button className="grid h-10 w-10 place-items-center rounded-full bg-[#f5f4f1] text-[#737792]">
              <PinIcon />
            </button>
          </div>

          <div className="mt-6 space-y-4">
            {pinnedRoots.length > 0 ? (
              pinnedRoots.map((root) => (
                <div key={root.id} className="flex items-start gap-4 rounded-[22px] bg-[#faf9f6] p-4">
                  <div className="grid h-12 w-12 shrink-0 place-items-center rounded-2xl bg-[#eceef6] text-[#737792]">
                    {iconForKind("document")}
                  </div>
                  <div className="min-w-0">
                    <p className="truncate text-base font-medium text-[#1f2723]">
                      {shortPath(root.path)}
                    </p>
                    <p className="mt-1 text-sm leading-6 text-[#727977]">
                      {root.fileCount.toLocaleString()} files • {statusLabel(root.status)} •{" "}
                      {syncStatusLabel(root.syncStatus)}
                    </p>
                  </div>
                </div>
              ))
            ) : (
              <div className="rounded-[22px] bg-[#faf9f6] p-5 text-sm leading-6 text-[#727977]">
                Add a source to start building your workspace.
              </div>
            )}
          </div>

          <div className="mt-6 rounded-[22px] border border-black/5 bg-[#fbfaf7] p-4">
            <div className="flex items-center justify-between gap-3">
              <p className="text-[0.72rem] uppercase tracking-[0.14em] text-[#7c8187]">
                Saved results
              </p>
              {savedResults.length > 0 ? (
                <span className="rounded-full bg-[#eff0f8] px-2.5 py-1 text-[0.68rem] uppercase tracking-[0.12em] text-[#676e88]">
                  {savedResults.length}
                </span>
              ) : null}
            </div>

            {savedResults.length > 0 ? (
              <div className="mt-4 space-y-3">
                {savedResults.map((result) => (
                  <div key={result.path} className="flex items-start gap-2 rounded-[18px] bg-white/80 p-3">
                    <button
                      className="min-w-0 flex-1 text-left"
                      onClick={() => void onShowSavedResult(result.path)}
                    >
                      <p className="truncate text-sm font-medium text-[#1f2723]">{result.name}</p>
                      <p className="mt-1 text-xs uppercase tracking-[0.12em] text-[#7b8186]">
                        {kindLabel(result.kind)} •{" "}
                        {result.modifiedAt ? formatRelativeDate(result.modifiedAt) : "Saved result"}
                      </p>
                      <p className="mt-1 truncate text-sm text-[#727977]">
                        {shortPath(result.path)}
                      </p>
                    </button>
                    <button
                      className="grid h-9 w-9 shrink-0 place-items-center rounded-full border border-black/5 bg-[#f5f4f1] text-[#7c8187] transition hover:bg-white"
                      onClick={() => onRemoveSavedResult(result.path)}
                      aria-label={`Remove ${result.name} from saved results`}
                    >
                      <BookmarkIcon />
                    </button>
                  </div>
                ))}
              </div>
            ) : (
              <p className="mt-4 text-sm leading-6 text-[#727977]">
                Save a search result to pin it here for quick preview access.
              </p>
            )}
          </div>

          <div
            ref={bindPreviewNode}
            tabIndex={-1}
            className="mt-6 rounded-[22px] border border-black/5 bg-[#fbfaf7] p-4 outline-none"
          >
            <p className="text-[0.72rem] uppercase tracking-[0.14em] text-[#7c8187]">
              Selected preview
            </p>
            {selectedFile ? (
              <div className="mt-4 space-y-4">
                <SelectedFilePreview
                  file={selectedFile}
                  previewUrl={selectedPreviewUrl}
                  query={query}
                  className="h-48 rounded-[20px]"
                />
                <div>
                  <p className="display-type text-[1.35rem] leading-8 text-[#222825]">
                    {selectedFile.name}
                  </p>
                  <p className="mt-2 text-sm leading-6 text-[#727977]">
                    {selectedFile.contentSnippet ??
                      "Run a search to surface extracted text snippets and file context here."}
                  </p>
                </div>
              </div>
            ) : (
              <p className="mt-4 text-sm leading-6 text-[#727977]">
                Search results and selected files will surface here with a richer preview.
              </p>
            )}
          </div>

          {message ? <p className="mt-4 text-sm text-[color:var(--danger)]">{message}</p> : null}
        </aside>
      </section>
    </div>
  );
}
