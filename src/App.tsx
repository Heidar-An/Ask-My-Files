import { useEffect, useMemo, useState } from "react";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import logoSrc from "./assets/logo.png";

type IndexedRoot = {
  id: number;
  path: string;
  status: string;
  fileCount: number;
  contentIndexedCount: number;
  contentPendingCount: number;
  semanticIndexedCount: number;
  semanticPendingCount: number;
  lastIndexedAt: number | null;
  lastError: string | null;
};

type IndexStatus = {
  jobId: number;
  rootId: number;
  phase: string;
  status: string;
  processed: number;
  total: number;
  currentPath: string | null;
  errors: string[];
  startedAt: number;
  finishedAt: number | null;
};

type SearchResult = {
  fileId: number;
  rootId: number;
  name: string;
  path: string;
  extension: string;
  kind: string;
  size: number;
  modifiedAt: number | null;
  indexedAt: number;
  score: number;
  semanticScore: number | null;
  matchReasons: string[];
  snippet: string | null;
  snippetSource: string | null;
};

type FileDetails = {
  fileId: number;
  rootId: number;
  rootPath: string;
  name: string;
  path: string;
  extension: string;
  kind: string;
  size: number;
  modifiedAt: number | null;
  indexedAt: number;
  previewPath: string | null;
  contentStatus: string | null;
  contentSnippet: string | null;
  contentSource: string | null;
  extractionError: string | null;
  semanticStatus: string | null;
  semanticModality: string | null;
  semanticModel: string | null;
  semanticSummary: string | null;
  semanticError: string | null;
};

type SearchRequest = {
  query: string;
  rootIds?: number[];
  limit?: number;
};

type ViewName = "home" | "results" | "sources" | "settings";

const SEARCH_PLACEHOLDERS = [
  "passport photo",
  "tax return 2024",
  "typescript config",
  "invoice from Acme",
];

const SUGGESTIONS = [
  "passport photo",
  "contract draft",
  "quarterly budget",
  "typescript config",
];

const panelClass =
  "rounded-[30px] border border-black/5 bg-white/78 shadow-[0_22px_60px_rgba(85,93,122,0.08)] backdrop-blur-xl";
const buttonClass =
  "inline-flex items-center justify-center gap-2 rounded-[18px] border border-black/8 bg-white/80 px-4 py-3 text-sm font-medium text-[#1f2723] transition hover:-translate-y-0.5 hover:bg-white";
const primaryButtonClass =
  "inline-flex items-center justify-center gap-2 rounded-[18px] bg-[#737792] px-5 py-3.5 text-sm font-medium text-white shadow-[0_16px_30px_rgba(115,119,146,0.18)] transition hover:-translate-y-0.5 hover:bg-[#676b86]";

export function App() {
  const [roots, setRoots] = useState<IndexedRoot[]>([]);
  const [statuses, setStatuses] = useState<Record<number, IndexStatus>>({});
  const [results, setResults] = useState<SearchResult[]>([]);
  const [selectedRootIds, setSelectedRootIds] = useState<number[]>([]);
  const [query, setQuery] = useState("");
  const [selectedFileId, setSelectedFileId] = useState<number | null>(null);
  const [selectedFile, setSelectedFile] = useState<FileDetails | null>(null);
  const [currentView, setCurrentView] = useState<ViewName>("home");
  const [isHydrating, setIsHydrating] = useState(true);
  const [isSearching, setIsSearching] = useState(false);
  const [message, setMessage] = useState<string | null>(null);

  const filteredRootIds = selectedRootIds.length > 0 ? selectedRootIds : undefined;
  const activeStatuses = useMemo(
    () => roots.map((root) => statuses[root.id]).filter(Boolean),
    [roots, statuses],
  );
  const runningIndexCount = activeStatuses.filter(
    (status) => status.status === "running",
  ).length;
  const totalFiles = roots.reduce((total, root) => total + root.fileCount, 0);
  const totalContentIndexed = roots.reduce(
    (total, root) => total + root.contentIndexedCount,
    0,
  );
  const totalContentPending = roots.reduce(
    (total, root) => total + root.contentPendingCount,
    0,
  );
  const totalSemanticIndexed = roots.reduce(
    (total, root) => total + root.semanticIndexedCount,
    0,
  );
  const totalSemanticPending = roots.reduce(
    (total, root) => total + root.semanticPendingCount,
    0,
  );
  const activeRootCount = roots.filter((root) => root.status === "ready").length;
  const enrichingRootCount = roots.filter(
    (root) => root.contentPendingCount > 0 || root.semanticPendingCount > 0,
  ).length;
  const selectedPreviewUrl =
    selectedFile?.previewPath ? convertFileSrc(selectedFile.previewPath) : null;
  const featuredResult = results[0] ?? null;
  const secondaryResults = results.slice(1, 4);
  const listResults = results.slice(4);
  const pinnedRoots = roots.slice(0, 3);
  const currentStatusText =
    runningIndexCount > 0
      ? `${runningIndexCount} source${runningIndexCount === 1 ? "" : "s"} indexing`
      : enrichingRootCount > 0
        ? `${enrichingRootCount} source${enrichingRootCount === 1 ? "" : "s"} enriching`
      : activeRootCount > 0
        ? "Index is active"
        : "Waiting for sources";

  useEffect(() => {
    void hydrate();
  }, []);

  useEffect(() => {
    const timer = window.setTimeout(() => {
      void runSearch(query, filteredRootIds);
    }, 120);

    return () => window.clearTimeout(timer);
  }, [query, filteredRootIds]);

  useEffect(() => {
    const timer = window.setInterval(() => {
      void refreshStatuses();
    }, 1500);

    return () => window.clearInterval(timer);
  }, []);

  useEffect(() => {
    if (query.trim().length > 0 && currentView !== "results") {
      setCurrentView("results");
    }
  }, [query, currentView]);

  useEffect(() => {
    if (results.length === 0) {
      setSelectedFileId(null);
      setSelectedFile(null);
      return;
    }

    if (
      selectedFileId === null ||
      !results.some((result) => result.fileId === selectedFileId)
    ) {
      setSelectedFileId(results[0].fileId);
    }
  }, [results, selectedFileId]);

  useEffect(() => {
    if (selectedFileId === null) {
      return;
    }

    void loadFileDetails(selectedFileId);
  }, [selectedFileId]);

  async function hydrate() {
    setIsHydrating(true);
    try {
      await Promise.all([loadRoots(), refreshStatuses(), runSearch("", undefined)]);
    } catch (error) {
      setMessage(getErrorMessage(error));
    } finally {
      setIsHydrating(false);
    }
  }

  async function loadRoots() {
    const nextRoots = await invoke<IndexedRoot[]>("list_index_roots");
    setRoots(nextRoots);
  }

  async function refreshStatuses() {
    const [nextStatuses, nextRoots] = await Promise.all([
      invoke<IndexStatus[]>("get_index_statuses"),
      invoke<IndexedRoot[]>("list_index_roots"),
    ]);

    const statusMap = nextStatuses.reduce<Record<number, IndexStatus>>((acc, status) => {
      acc[status.rootId] = status;
      return acc;
    }, {});

    setStatuses(statusMap);
    setRoots(nextRoots);
  }

  async function runSearch(nextQuery: string, rootIds?: number[]) {
    setIsSearching(true);
    try {
      const payload: SearchRequest = {
        query: nextQuery,
        limit: 60,
      };

      if (rootIds && rootIds.length > 0) {
        payload.rootIds = rootIds;
      }

      const nextResults = await invoke<SearchResult[]>("search_files", { request: payload });
      setResults(nextResults);
      setMessage(null);
    } catch (error) {
      setMessage(getErrorMessage(error));
    } finally {
      setIsSearching(false);
    }
  }

  async function loadFileDetails(fileId: number) {
    try {
      const details = await invoke<FileDetails>("get_file_details", { fileId });
      setSelectedFile(details);
    } catch (error) {
      setMessage(getErrorMessage(error));
    }
  }

  async function handleAddFolder() {
    try {
      const selected = await openDialog({
        directory: true,
        multiple: false,
        title: "Choose a folder to index",
      });

      if (!selected || Array.isArray(selected)) {
        return;
      }

      const root = await invoke<IndexedRoot>("add_index_root", { path: selected });
      await invoke("start_index", { rootId: root.id });
      setCurrentView("sources");
      await Promise.all([loadRoots(), refreshStatuses(), runSearch(query, filteredRootIds)]);
    } catch (error) {
      setMessage(getErrorMessage(error));
    }
  }

  async function handleRescan(rootId: number) {
    try {
      await invoke("start_index", { rootId });
      await refreshStatuses();
    } catch (error) {
      setMessage(getErrorMessage(error));
    }
  }

  async function handleRescanAll() {
    try {
      await Promise.all(roots.map((root) => invoke("start_index", { rootId: root.id })));
      await refreshStatuses();
    } catch (error) {
      setMessage(getErrorMessage(error));
    }
  }

  async function handleRemoveRoot(rootId: number) {
    try {
      await invoke("remove_index_root", { rootId });
      const nextSelected = selectedRootIds.filter((id) => id !== rootId);
      setSelectedRootIds(nextSelected);
      await Promise.all([
        loadRoots(),
        refreshStatuses(),
        runSearch(query, nextSelected.length > 0 ? nextSelected : undefined),
      ]);
    } catch (error) {
      setMessage(getErrorMessage(error));
    }
  }

  async function handleOpenFile(path: string) {
    try {
      await invoke("open_file", { path });
    } catch (error) {
      setMessage(getErrorMessage(error));
    }
  }

  async function handleRevealFile(path: string) {
    try {
      await invoke("reveal_file", { path });
    } catch (error) {
      setMessage(getErrorMessage(error));
    }
  }

  function toggleRoot(rootId: number) {
    setSelectedRootIds((current) =>
      current.includes(rootId)
        ? current.filter((id) => id !== rootId)
        : [...current, rootId],
    );
  }

  const headerTitle =
    currentView === "results" && query.trim().length > 0
      ? "Search analysis"
      : currentView === "sources"
        ? "Sources & indexing"
        : currentView === "settings"
          ? "Workspace settings"
          : "Workspace";

  return (
    <div className="min-h-dvh p-4 md:p-6">
      <div className="grid min-h-[calc(100dvh-2rem)] grid-cols-1 gap-4 xl:grid-cols-[290px_minmax(0,1fr)]">
        <aside className={cx(panelClass, "flex flex-col overflow-hidden p-5")}>
          <div className="flex items-start gap-4">
            <img src={logoSrc} alt="Mira" className="h-12 w-12 shrink-0 rounded-2xl object-cover" />
            <div>
              <p className="display-type text-[1.1rem] leading-6 text-[#1d2320]">
                Mira
              </p>
              <p className="mt-1 text-sm tracking-[0.16em] text-[#80858a] uppercase">
                AI-powered search
              </p>
            </div>
          </div>

          <button className={cx(primaryButtonClass, "mt-8 w-full")} onClick={() => void handleAddFolder()}>
            <PlusIcon />
            Add source
          </button>

          <nav className="mt-8 space-y-2">
            <SidebarLink
              label="Home"
              active={currentView === "home"}
              onClick={() => setCurrentView("home")}
              icon={<HomeIcon />}
            />
            <SidebarLink
              label="Results"
              active={currentView === "results"}
              onClick={() => setCurrentView("results")}
              icon={<ResultsIcon />}
              badge={results.length > 0 ? results.length : undefined}
            />
            <SidebarLink
              label="Sources"
              active={currentView === "sources"}
              onClick={() => setCurrentView("sources")}
              icon={<DatabaseIcon />}
              badge={roots.length > 0 ? roots.length : undefined}
            />
            <SidebarLink
              label="Settings"
              active={currentView === "settings"}
              onClick={() => setCurrentView("settings")}
              icon={<SettingsIcon />}
            />
          </nav>

          <div className="mt-8 rounded-[24px] bg-[#f4f1ec] p-4">
            <p className="text-[0.72rem] uppercase tracking-[0.14em] text-[#7c8187]">
              Current status
            </p>
            <p className="display-type mt-2 text-[1.35rem] leading-8 text-[#222825]">
              {currentStatusText}
            </p>
            <p className="mt-2 text-sm leading-6 text-[#68716d]">
              {runningIndexCount > 0
                ? "Metadata indexing is happening in the background and deeper passes will continue afterward."
                : enrichingRootCount > 0
                  ? "Metadata is ready, while content extraction and semantic enrichment are still catching up."
                  : "Your local workspace is ready for metadata, extracted-text, and visual-semantic search."}
            </p>
          </div>

          <div className="mt-auto rounded-[24px] border border-black/5 bg-white/72 p-4">
            <div className="flex items-center gap-3">
              <div className="grid h-12 w-12 place-items-center rounded-2xl bg-[#eef0f8] text-[#737792]">
                <UserIcon />
              </div>
              <div>
                <p className="font-medium text-[#1d2320]">Mira</p>
                <p className="text-sm text-[#7b8280]">
                  {activeRootCount} active source{activeRootCount === 1 ? "" : "s"}
                </p>
              </div>
            </div>
          </div>
        </aside>

        <main className={cx(panelClass, "overflow-hidden")}>
          <div className="flex h-full flex-col">
            <header className="border-b border-black/5 px-5 py-4 sm:px-7">
              <div className="flex flex-col gap-4 lg:flex-row lg:items-center lg:justify-between">
                <div className="flex min-w-0 flex-1 items-center gap-4">
                  <div className="relative min-w-0 flex-1">
                    <SearchIcon className="absolute left-5 top-1/2 h-5 w-5 -translate-y-1/2 text-[#7c8187]" />
                    <input
                      className="w-full rounded-[22px] border border-black/5 bg-[#f7f3ed] py-4 pl-14 pr-5 text-[1.02rem] text-[#1e2522] outline-none transition focus:border-[#d7d1c7] focus:bg-white focus:ring-4 focus:ring-[#7377921a]"
                      value={query}
                      onChange={(event) => setQuery(event.target.value)}
                      placeholder={
                        SEARCH_PLACEHOLDERS[
                          Math.floor(Date.now() / 1000) % SEARCH_PLACEHOLDERS.length
                        ]
                      }
                    />
                  </div>
                </div>

                <div className="flex items-center gap-2 text-sm text-[#6f757d]">
                  <TopChip
                    label="Workspace"
                    active={currentView === "home" || currentView === "results"}
                    onClick={() => setCurrentView(query.trim().length > 0 ? "results" : "home")}
                  />
                  <TopChip label="Sources" active={currentView === "sources"} onClick={() => setCurrentView("sources")} />
                  <TopChip label="Settings" active={currentView === "settings"} onClick={() => setCurrentView("settings")} />
                  <button className="ml-2 grid h-10 w-10 place-items-center rounded-full border border-black/5 bg-white/70">
                    <HelpIcon />
                  </button>
                  <button className="grid h-10 w-10 place-items-center rounded-full border border-black/5 bg-white/70">
                    <UserIcon />
                  </button>
                </div>
              </div>
            </header>

            <div className="min-h-0 flex-1 overflow-auto px-5 py-6 sm:px-7 sm:py-7">
              {currentView === "home" && (
                <HomeView
                  roots={roots}
                  totalFiles={totalFiles}
                  runningIndexCount={runningIndexCount}
                  query={query}
                  setQuery={setQuery}
                  setCurrentView={setCurrentView}
                  selectedFile={selectedFile}
                  selectedPreviewUrl={selectedPreviewUrl}
                  pinnedRoots={pinnedRoots}
                  message={message}
                />
              )}

              {currentView === "results" && (
                <ResultsView
                  query={query}
                  results={results}
                  featuredResult={featuredResult}
                  secondaryResults={secondaryResults}
                  listResults={listResults}
                  selectedFile={selectedFile}
                  selectedPreviewUrl={selectedPreviewUrl}
                  isHydrating={isHydrating}
                  isSearching={isSearching}
                  onSelectResult={setSelectedFileId}
                  onOpenFile={handleOpenFile}
                  onRevealFile={handleRevealFile}
                  message={message}
                />
              )}

              {currentView === "sources" && (
                <SourcesView
                  roots={roots}
                  statuses={statuses}
                  totalFiles={totalFiles}
                  totalContentIndexed={totalContentIndexed}
                  totalContentPending={totalContentPending}
                  totalSemanticIndexed={totalSemanticIndexed}
                  totalSemanticPending={totalSemanticPending}
                  runningIndexCount={runningIndexCount}
                  selectedRootIds={selectedRootIds}
                  toggleRoot={toggleRoot}
                  onAddFolder={handleAddFolder}
                  onRescan={handleRescan}
                  onRescanAll={handleRescanAll}
                  onRemoveRoot={handleRemoveRoot}
                />
              )}

              {currentView === "settings" && (
                <SettingsView
                  headerTitle={headerTitle}
                  totalFiles={totalFiles}
                  roots={roots}
                  currentStatusText={currentStatusText}
                />
              )}
            </div>
          </div>
        </main>
      </div>
    </div>
  );
}

function HomeView({
  roots,
  totalFiles,
  runningIndexCount,
  query,
  setQuery,
  setCurrentView,
  selectedFile,
  selectedPreviewUrl,
  pinnedRoots,
  message,
}: {
  roots: IndexedRoot[];
  totalFiles: number;
  runningIndexCount: number;
  query: string;
  setQuery: (value: string) => void;
  setCurrentView: (view: ViewName) => void;
  selectedFile: FileDetails | null;
  selectedPreviewUrl: string | null;
  pinnedRoots: IndexedRoot[];
  message: string | null;
}) {
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
              <SparkleIcon className="h-6 w-6 text-[#737792]" />
              <input
                className="w-full bg-transparent text-[1.15rem] text-[#222825] outline-none placeholder:text-[#9da2a6]"
                value={query}
                onChange={(event) => setQuery(event.target.value)}
                placeholder="Ask your workspace anything..."
              />
            </div>
            <button className={cx(primaryButtonClass, "min-w-[180px] rounded-[22px] px-6")} onClick={() => setCurrentView("results")}>
              Analyze
              <ArrowRightIcon />
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
                    <p className="truncate text-base font-medium text-[#1f2723]">{shortPath(root.path)}</p>
                    <p className="mt-1 text-sm leading-6 text-[#727977]">
                      {root.fileCount.toLocaleString()} files • {statusLabel(root.status)}
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
            <p className="text-[0.72rem] uppercase tracking-[0.14em] text-[#7c8187]">
              Selected preview
            </p>
            {selectedFile ? (
              <div className="mt-4 space-y-4">
                <SelectedFilePreview
                  file={selectedFile}
                  previewUrl={selectedPreviewUrl}
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

function ResultsView({
  query,
  results,
  featuredResult,
  secondaryResults,
  listResults,
  selectedFile,
  selectedPreviewUrl,
  isHydrating,
  isSearching,
  onSelectResult,
  onOpenFile,
  onRevealFile,
  message,
}: {
  query: string;
  results: SearchResult[];
  featuredResult: SearchResult | null;
  secondaryResults: SearchResult[];
  listResults: SearchResult[];
  selectedFile: FileDetails | null;
  selectedPreviewUrl: string | null;
  isHydrating: boolean;
  isSearching: boolean;
  onSelectResult: (fileId: number) => void;
  onOpenFile: (path: string) => Promise<void>;
  onRevealFile: (path: string) => Promise<void>;
  message: string | null;
}) {
  const emptyState = isHydrating
    ? "Loading your workspace..."
    : query.trim().length === 0
      ? "Start with a query like “passport photo” or “tax return 2024”."
      : "No files matched that query yet.";

  return (
    <div className="space-y-6">
      <section className="px-1 pt-2">
        <div className="flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between">
          <div>
            <p className="text-[0.82rem] uppercase tracking-[0.22em] text-[#727792]">
              Search query analysis
            </p>
            <h1 className="display-type mt-4 max-w-4xl text-[clamp(2.3rem,5vw,4.1rem)] leading-[0.98] text-[#242b28]">
              {query.trim().length > 0 ? query : "Explore your workspace"}
            </h1>
            <p className="mt-4 flex flex-wrap items-center gap-3 text-[1.05rem] text-[#686f6c]">
              <span className="inline-flex items-center gap-2 text-[#272d2a]">
                <SparkleIcon className="h-5 w-5 text-[#737792]" />
                Hybrid lexical + visual matches
              </span>
              <span className="rounded-full bg-[#eaecf8] px-3 py-1 text-[0.72rem] uppercase tracking-[0.14em] text-[#5d647f]">
                High confidence
              </span>
              {isSearching ? (
                <span className="rounded-full bg-[#f0f1f4] px-3 py-1 text-[0.72rem] uppercase tracking-[0.14em] text-[#7b8186]">
                  Searching
                </span>
              ) : null}
            </p>
          </div>

          <div className="flex flex-wrap gap-3">
            <button className={buttonClass}>
              <FilterIcon />
              Filter
            </button>
            <button className={buttonClass}>
              <ShareIcon />
              Share
            </button>
          </div>
        </div>
      </section>

      {results.length === 0 ? (
        <div className={cx(panelClass, "p-8 text-center")}>
          <p className="display-type text-[2rem] text-[#262d2a]">{emptyState}</p>
          {message ? <p className="mt-3 text-sm text-[color:var(--danger)]">{message}</p> : null}
        </div>
      ) : (
        <>
          <section className="grid gap-4 xl:grid-cols-[1.55fr_0.9fr]">
            <article className={cx(panelClass, "p-7")}>
              {featuredResult ? (
                <button className="w-full text-left" onClick={() => onSelectResult(featuredResult.fileId)}>
                  <div className="flex flex-wrap items-center gap-3 text-sm text-[#747a80]">
                    <span className="rounded-full bg-[#eff0f8] px-3 py-1 text-[0.78rem] font-medium text-[#676e88]">
                      {kindLabel(featuredResult.kind)}
                    </span>
                    <span>{featuredResult.extension.toUpperCase() || "FILE"}</span>
                    <span>{featuredResult.modifiedAt ? formatDate(featuredResult.modifiedAt) : "Unknown date"}</span>
                  </div>

                  <div className="mt-4 flex items-start justify-between gap-5">
                    <h2 className="display-type max-w-3xl text-[2rem] leading-tight text-[#202724]">
                      {featuredResult.name}
                    </h2>
                    <div className="text-right">
                      <p className="display-type text-[3.3rem] leading-none text-[#e1e3ea]">
                        {Math.min(99, Math.max(61, Math.round(featuredResult.score / 4)))}
                      </p>
                      <p className="text-xs uppercase tracking-[0.16em] text-[#7b8186]">
                        Similarity
                      </p>
                    </div>
                  </div>

                  <p className="mt-5 max-w-4xl text-[1.05rem] leading-8 text-[#666d6a]">
                    {featuredResult.snippet ??
                      "This file surfaced through strong filename and metadata evidence. Open it to inspect the full contents."}
                  </p>

                  <div className="mt-6 flex flex-wrap items-center gap-4 text-sm text-[#5d647f]">
                    <span className="inline-flex items-center gap-2">
                      <ArrowRightIcon />
                      Open preview
                    </span>
                    <span className="inline-flex items-center gap-2 text-[#7b8186]">
                      <BookmarkIcon />
                      Save result
                    </span>
                  </div>
                </button>
              ) : null}
            </article>

            <aside className="grid gap-4 md:grid-cols-3 xl:grid-cols-1">
              {secondaryResults.map((result) => (
                <button
                  key={result.fileId}
                  className={cx(panelClass, "p-5 text-left")}
                  onClick={() => onSelectResult(result.fileId)}
                >
                  <div className="flex items-start justify-between gap-3">
                    <p className="text-lg font-semibold text-[#737792]">
                      {Math.min(99, Math.max(48, Math.round(result.score / 4)))}%
                    </p>
                    <span className="text-[#7b8186]">{iconForKind(result.kind)}</span>
                  </div>
                  <p className="display-type mt-5 text-[1.55rem] leading-tight text-[#202724]">
                    {result.name}
                  </p>
                  <p className="mt-4 text-sm leading-6 text-[#666d6a]">
                    {result.snippet ?? shortPath(result.path)}
                  </p>
                  <div className="mt-4 flex flex-wrap gap-2">
                    {result.matchReasons.slice(0, 2).map((reason) => (
                      <span
                        key={`${result.fileId}-${reason}`}
                        className="rounded-full bg-[#f3f1ea] px-2.5 py-1 text-[0.68rem] uppercase tracking-[0.12em] text-[#686f88]"
                      >
                        {reason}
                      </span>
                    ))}
                  </div>
                </button>
              ))}
            </aside>
          </section>

          <section className="grid gap-4 xl:grid-cols-[1.2fr_0.8fr]">
            <article className={cx(panelClass, "p-6")}>
              <div className="flex items-center gap-3">
                <ListIcon />
                <h3 className="display-type text-[1.6rem] text-[#202724]">Explorer</h3>
              </div>

              <div className="mt-5 space-y-3">
                {(listResults.length > 0 ? listResults : results).map((result) => (
                  <button
                    key={result.fileId}
                    className="flex w-full items-start gap-4 rounded-[22px] border border-black/5 bg-white/70 p-4 text-left transition hover:-translate-y-0.5 hover:bg-white"
                    onClick={() => onSelectResult(result.fileId)}
                  >
                    <div className="grid h-12 w-12 shrink-0 place-items-center rounded-2xl bg-[#eef0f6] text-[#737792]">
                      {iconForKind(result.kind)}
                    </div>
                    <div className="min-w-0 flex-1">
                      <p className="text-[0.72rem] uppercase tracking-[0.14em] text-[#7b8186]">
                        {kindLabel(result.kind)} • {result.modifiedAt ? formatRelativeDate(result.modifiedAt) : "Recently indexed"}
                      </p>
                      <p className="display-type mt-2 text-[1.3rem] leading-8 text-[#202724]">
                        {result.name}
                      </p>
                      <p className="mt-2 text-sm leading-6 text-[#666d6a]">
                        {result.snippet ?? result.path}
                      </p>
                      <div className="mt-3 flex flex-wrap gap-2">
                        {result.matchReasons.slice(0, 3).map((reason) => (
                          <span
                            key={`${result.fileId}-${reason}`}
                            className="rounded-full bg-[#f3f1ea] px-2.5 py-1 text-[0.68rem] uppercase tracking-[0.12em] text-[#686f88]"
                          >
                            {reason}
                          </span>
                        ))}
                      </div>
                    </div>
                  </button>
                ))}
              </div>
            </article>

            <aside className={cx(panelClass, "p-6")}>
              <div className="flex items-center justify-between gap-3">
                <h3 className="display-type text-[1.6rem] text-[#202724]">Selected file</h3>
                {selectedFile ? (
                  <span className="rounded-full bg-[#eff0f8] px-3 py-1 text-[0.72rem] uppercase tracking-[0.14em] text-[#676e88]">
                    {detailStatusLabel(selectedFile)}
                  </span>
                ) : null}
              </div>

              {selectedFile ? (
                <div className="mt-5 space-y-5">
                  <SelectedFilePreview
                    file={selectedFile}
                    previewUrl={selectedPreviewUrl}
                    className="h-60 rounded-[24px]"
                  />

                  <div>
                    <p className="display-type text-[1.8rem] leading-tight text-[#202724]">
                      {selectedFile.name}
                    </p>
                    <p className="mt-3 text-sm leading-7 text-[#666d6a]">
                      {selectedFile.contentSnippet ??
                        selectedFile.semanticSummary ??
                        "This file is available in your workspace. Use the actions below to open or reveal it."}
                    </p>
                  </div>

                  <div className="grid gap-3 text-sm text-[#666d6a]">
                    <InfoRow label="Location" value={selectedFile.path} />
                    <InfoRow label="Root" value={selectedFile.rootPath} />
                    <InfoRow label="Size" value={formatBytes(selectedFile.size)} />
                    <InfoRow
                      label="Modified"
                      value={
                        selectedFile.modifiedAt ? formatDate(selectedFile.modifiedAt) : "Unknown"
                      }
                    />
                    {selectedFile.semanticModality ? (
                      <InfoRow label="Semantic mode" value={selectedFile.semanticModality} />
                    ) : null}
                    {selectedFile.semanticModel ? (
                      <InfoRow label="Semantic model" value={selectedFile.semanticModel} />
                    ) : null}
                    {selectedFile.contentSource ? (
                      <InfoRow label="Snippet source" value={selectedFile.contentSource} />
                    ) : null}
                  </div>

                  {selectedFile.extractionError ? (
                    <div className="rounded-[20px] bg-[#fff3ef] px-4 py-3 text-sm text-[color:var(--danger)]">
                      {selectedFile.extractionError}
                    </div>
                  ) : null}
                  {selectedFile.semanticError ? (
                    <div className="rounded-[20px] bg-[#fff7e8] px-4 py-3 text-sm text-[#8b6322]">
                      {selectedFile.semanticError}
                    </div>
                  ) : null}

                  <div className="flex flex-wrap gap-3">
                    <button className={primaryButtonClass} onClick={() => void onOpenFile(selectedFile.path)}>
                      Open file
                    </button>
                    <button className={buttonClass} onClick={() => void onRevealFile(selectedFile.path)}>
                      Reveal in Finder
                    </button>
                  </div>
                </div>
              ) : (
                <div className="mt-5 rounded-[24px] bg-[#faf9f6] p-5 text-sm leading-7 text-[#666d6a]">
                  Select a result to inspect the file, preview extracted text, and launch quick actions.
                </div>
              )}

              {message ? <p className="mt-4 text-sm text-[color:var(--danger)]">{message}</p> : null}
            </aside>
          </section>
        </>
      )}
    </div>
  );
}

function SourcesView({
  roots,
  statuses,
  totalFiles,
  totalContentIndexed,
  totalContentPending,
  totalSemanticIndexed,
  totalSemanticPending,
  runningIndexCount,
  selectedRootIds,
  toggleRoot,
  onAddFolder,
  onRescan,
  onRescanAll,
  onRemoveRoot,
}: {
  roots: IndexedRoot[];
  statuses: Record<number, IndexStatus>;
  totalFiles: number;
  totalContentIndexed: number;
  totalContentPending: number;
  totalSemanticIndexed: number;
  totalSemanticPending: number;
  runningIndexCount: number;
  selectedRootIds: number[];
  toggleRoot: (rootId: number) => void;
  onAddFolder: () => Promise<void>;
  onRescan: (rootId: number) => Promise<void>;
  onRescanAll: () => Promise<void>;
  onRemoveRoot: (rootId: number) => Promise<void>;
}) {
  return (
    <div className="space-y-6">
      <section className="flex flex-col gap-4 px-1 pt-2 lg:flex-row lg:items-start lg:justify-between">
        <div>
          <p className="text-[0.82rem] uppercase tracking-[0.22em] text-[#727792]">
            Source management
          </p>
          <h1 className="display-type mt-4 text-[clamp(2.5rem,5vw,3.8rem)] leading-[0.96] text-[#242b28]">
            Sources & indexing
          </h1>
          <p className="mt-4 max-w-3xl text-[1.08rem] leading-8 text-[#6a716d]">
            Manage local folders, monitor indexing progress, and keep your workspace ready for natural-language search.
          </p>
        </div>

        <div className="flex flex-wrap gap-3">
          <button className={buttonClass} onClick={() => void onRescanAll()}>
            Re-index all
          </button>
          <button className={primaryButtonClass} onClick={() => void onAddFolder()}>
            Connect source
          </button>
        </div>
      </section>

      <section className="grid gap-4 xl:grid-cols-3">
        <MetricPanel
          title="Metadata indexed"
          value={formatCompact(totalFiles)}
          note={`${roots.length} sources connected`}
          bar={100}
        />
        <MetricPanel
          title="Content extracted"
          value={formatPassValue(
            totalContentIndexed,
            totalContentIndexed + totalContentPending,
          )}
          note={
            totalContentPending > 0
              ? `${totalContentPending.toLocaleString()} files still waiting for text extraction`
              : "All supported files are content-searchable"
          }
          segmented
        />
        <MetricPanel
          title="Semantic enriched"
          value={formatPassValue(
            totalSemanticIndexed,
            totalSemanticIndexed + totalSemanticPending,
          )}
          note={
            runningIndexCount > 0
              ? "Fast metadata pass is still running"
              : totalSemanticPending > 0
                ? `${totalSemanticPending.toLocaleString()} files still waiting for semantic enrichment`
                : "Semantic enrichment is up to date"
          }
          indicator
        />
      </section>

      <section className={cx(panelClass, "p-6")}>
        <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
          <h2 className="display-type text-[1.8rem] text-[#202724]">Active sources</h2>
          <div className="text-sm text-[#727977]">
            {selectedRootIds.length > 0
              ? `${selectedRootIds.length} source filter${selectedRootIds.length === 1 ? "" : "s"} active`
              : "Showing all connected sources"}
          </div>
        </div>

        <div className="mt-5 space-y-4">
          {roots.length > 0 ? (
            roots.map((root) => {
              const status = statuses[root.id];
              const metadataProgress =
                status && status.total > 0
                  ? Math.round((status.processed / status.total) * 100)
                  : root.status === "ready"
                    ? 100
                    : 0;
              const contentTotal = root.contentIndexedCount + root.contentPendingCount;
              const semanticTotal = root.semanticIndexedCount + root.semanticPendingCount;

              return (
                <article
                  key={root.id}
                  className={cx(
                    "grid gap-4 rounded-[24px] border border-black/5 bg-white/72 p-5 transition lg:grid-cols-[auto_minmax(0,1fr)_auto]",
                    selectedRootIds.includes(root.id) && "ring-2 ring-[#7377921f]",
                  )}
                >
                  <button
                    className="grid h-16 w-16 place-items-center rounded-[22px] bg-[#eceef6] text-[#737792]"
                    onClick={() => toggleRoot(root.id)}
                  >
                    <FolderIcon />
                  </button>

                  <div className="min-w-0">
                    <div className="flex flex-col gap-2 lg:flex-row lg:items-center lg:justify-between">
                      <div>
                        <p className="display-type text-[1.45rem] leading-8 text-[#202724]">
                          {shortPath(root.path)}
                        </p>
                        <p className="mt-1 text-sm leading-6 text-[#6d7470]">
                          {root.path}
                        </p>
                      </div>
                      <span className={statusPillClass(root.status)}>{statusLabel(root.status)}</span>
                    </div>

                    <div className="mt-4">
                      <div className="flex items-center justify-between gap-3 text-sm text-[#70767a]">
                        <span>Pipeline progress</span>
                        <span>{rootPipelineLabel(root, status)}</span>
                      </div>
                      <div className="mt-3 space-y-3">
                        <PassProgressRow
                          label="Metadata"
                          value={`${status && status.status === "running" ? metadataProgress : root.fileCount > 0 ? 100 : 0}%`}
                          progress={status && status.status === "running" ? metadataProgress : root.fileCount > 0 ? 100 : 0}
                          tone="primary"
                        />
                        <PassProgressRow
                          label="Content"
                          value={formatPassValue(root.contentIndexedCount, contentTotal)}
                          progress={stagePercent(root.contentIndexedCount, contentTotal)}
                          tone="secondary"
                        />
                        <PassProgressRow
                          label="Semantic"
                          value={formatPassValue(root.semanticIndexedCount, semanticTotal)}
                          progress={stagePercent(root.semanticIndexedCount, semanticTotal)}
                          tone="muted"
                        />
                      </div>
                    </div>

                    <div className="mt-4 flex flex-wrap gap-4 text-sm text-[#6d7470]">
                      <span>{root.fileCount.toLocaleString()} files</span>
                      <span>{root.contentIndexedCount.toLocaleString()} content-ready</span>
                      <span>{root.semanticIndexedCount.toLocaleString()} semantic-ready</span>
                      <span>{root.lastIndexedAt ? formatDate(root.lastIndexedAt) : "Not indexed yet"}</span>
                      {status?.currentPath ? <span className="truncate">{status.currentPath}</span> : null}
                    </div>

                    {root.lastError ? (
                      <p className="mt-3 text-sm text-[color:var(--danger)]">{root.lastError}</p>
                    ) : null}
                  </div>

                  <div className="flex items-start gap-2 lg:flex-col">
                    <button className={buttonClass} onClick={() => void onRescan(root.id)}>
                      <RefreshIcon />
                      Rescan
                    </button>
                    <button
                      className={cx(buttonClass, "text-[color:var(--danger)]")}
                      onClick={() => void onRemoveRoot(root.id)}
                    >
                      Remove
                    </button>
                  </div>
                </article>
              );
            })
          ) : (
            <div className="rounded-[24px] bg-[#faf9f6] p-8 text-center text-[#6d7470]">
              <p className="display-type text-[1.8rem] text-[#202724]">No sources connected yet.</p>
              <p className="mt-3 text-base leading-7">
                Add a local folder to start building your workspace index.
              </p>
            </div>
          )}
        </div>
      </section>
    </div>
  );
}

function SettingsView({
  headerTitle,
  totalFiles,
  roots,
  currentStatusText,
}: {
  headerTitle: string;
  totalFiles: number;
  roots: IndexedRoot[];
  currentStatusText: string;
}) {
  return (
    <div className="space-y-6">
      <section className="px-1 pt-2">
        <p className="text-[0.82rem] uppercase tracking-[0.22em] text-[#727792]">
          {headerTitle}
        </p>
        <h1 className="display-type mt-4 text-[clamp(2.5rem,5vw,3.8rem)] leading-[0.96] text-[#242b28]">
          Tune the workspace behavior
        </h1>
        <p className="mt-4 max-w-3xl text-[1.08rem] leading-8 text-[#6a716d]">
          These settings are placeholders for the next product layer, but the cards already reflect the live state of your local workspace.
        </p>
      </section>

      <section className="grid gap-4 xl:grid-cols-3">
        <OverviewCard label="Workspace status" value={currentStatusText} meta="Live state from the local index" />
        <OverviewCard label="Indexed files" value={totalFiles.toLocaleString()} meta="Searchable metadata and extracted text" />
        <OverviewCard label="Connected sources" value={roots.length.toLocaleString()} meta="Local folders in your workspace" />
      </section>

      <section className={cx(panelClass, "p-6")}>
        <h2 className="display-type text-[1.8rem] text-[#202724]">What comes next</h2>
        <div className="mt-5 grid gap-4 md:grid-cols-2 xl:grid-cols-4">
          {[
            "Semantic ranking powered by LanceDB",
            "Richer previews for PDFs and Office docs",
            "Saved search collections and pinned files",
            "Incremental watchers for real-time refresh",
          ].map((item) => (
            <div key={item} className="rounded-[24px] bg-[#faf9f6] p-5 text-sm leading-7 text-[#6d7470]">
              {item}
            </div>
          ))}
        </div>
      </section>
    </div>
  );
}

function SidebarLink({
  label,
  icon,
  active,
  onClick,
  badge,
}: {
  label: string;
  icon: React.ReactNode;
  active: boolean;
  onClick: () => void;
  badge?: number;
}) {
  return (
    <button
      className={cx(
        "flex w-full items-center justify-between rounded-[18px] px-4 py-3.5 text-left text-[1.02rem] transition",
        active ? "bg-white text-[#4d5577] shadow-[0_8px_20px_rgba(85,93,122,0.08)]" : "text-[#595f63] hover:bg-white/70",
      )}
      onClick={onClick}
    >
      <span className="flex items-center gap-3">
        <span className={cx("text-[#737792]", active && "text-[#5a6386]")}>{icon}</span>
        {label}
      </span>
      {badge ? (
        <span className="rounded-full bg-[#eef0f5] px-2 py-0.5 text-xs text-[#737792]">
          {badge}
        </span>
      ) : null}
    </button>
  );
}

function TopChip({
  label,
  active,
  onClick,
}: {
  label: string;
  active: boolean;
  onClick: () => void;
}) {
  return (
    <button
      className={cx(
        "rounded-full px-4 py-2 transition",
        active ? "bg-[#f3f0ea] text-[#232a27]" : "text-[#73797e] hover:bg-[#f7f4ef]",
      )}
      onClick={onClick}
    >
      {label}
    </button>
  );
}

function OverviewCard({
  label,
  value,
  meta,
}: {
  label: string;
  value: string;
  meta: string;
}) {
  return (
    <div className="rounded-[24px] bg-[#faf9f6] p-5">
      <p className="text-[0.75rem] uppercase tracking-[0.14em] text-[#7c8187]">{label}</p>
      <p className="display-type mt-3 text-[1.9rem] leading-tight text-[#202724]">{value}</p>
      <p className="mt-3 text-sm leading-6 text-[#6d7470]">{meta}</p>
    </div>
  );
}

function MetricPanel({
  title,
  value,
  note,
  bar,
  segmented,
  indicator,
}: {
  title: string;
  value: string;
  note: string;
  bar?: number;
  segmented?: boolean;
  indicator?: boolean;
}) {
  return (
    <div className={cx(panelClass, "p-6")}>
      <p className="text-[0.75rem] uppercase tracking-[0.14em] text-[#7c8187]">{title}</p>
      <div className="mt-4 flex items-end gap-3">
        <p className="display-type text-[2.3rem] leading-none text-[#202724]">{value}</p>
        {indicator ? <span className="mb-1 h-4 w-4 rounded-full bg-[#74d2b8]" /> : null}
      </div>
      <p className="mt-3 text-sm leading-6 text-[#6d7470]">{note}</p>
      {typeof bar === "number" ? (
        <div className="mt-5 h-2 overflow-hidden rounded-full bg-[#e6e7ea]">
          <div className="h-full rounded-full bg-[#737792]" style={{ width: `${bar}%` }} />
        </div>
      ) : null}
      {segmented ? (
        <div className="mt-5 grid grid-cols-4 gap-2">
          {[32, 28, 22, 18].map((width) => (
            <div key={width} className="h-2 rounded-full bg-[#dfe1e7]" style={{ width: `${width * 3}%` }} />
          ))}
        </div>
      ) : null}
    </div>
  );
}

function PassProgressRow({
  label,
  value,
  progress,
  tone,
}: {
  label: string;
  value: string;
  progress: number;
  tone: "primary" | "secondary" | "muted";
}) {
  const fillClass =
    tone === "primary"
      ? "bg-[#737792]"
      : tone === "secondary"
        ? "bg-[#8e93ad]"
        : "bg-[#c8ccd9]";

  return (
    <div>
      <div className="flex items-center justify-between gap-3 text-sm text-[#70767a]">
        <span>{label}</span>
        <span>{value}</span>
      </div>
      <div className="mt-2 h-2 overflow-hidden rounded-full bg-[#e5e6ea]">
        <div
          className={cx("h-full rounded-full transition-[width]", fillClass)}
          style={{ width: `${Math.max(progress, progress > 0 ? 6 : 0)}%` }}
        />
      </div>
    </div>
  );
}

function InfoRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="grid gap-1">
      <dt className="text-[0.72rem] uppercase tracking-[0.14em] text-[#7c8187]">{label}</dt>
      <dd className="break-words text-[#252c29]">{value}</dd>
    </div>
  );
}

function SelectedFilePreview({
  file,
  previewUrl,
  className,
}: {
  file: FileDetails;
  previewUrl: string | null;
  className: string;
}) {
  const previewType = getPreviewType(file, previewUrl);

  if (previewType === "image") {
    return (
      <div className={cx("overflow-hidden bg-[#eef0f6]", className)}>
        <img src={previewUrl ?? undefined} alt={file.name} className="h-full w-full object-cover" />
      </div>
    );
  }

  if (previewType === "pdf") {
    return (
      <div className={cx("overflow-hidden bg-[#eef0f6]", className)}>
        <iframe
          src={previewUrl ?? undefined}
          title={file.name}
          className="h-full w-full border-0 bg-white"
        />
      </div>
    );
  }

  return (
    <div className={cx("grid place-items-center bg-[#eef0f6] text-[#737792]", className)}>
      {iconForKind(file.kind)}
    </div>
  );
}

function getPreviewType(file: FileDetails, previewUrl: string | null) {
  if (!previewUrl) {
    return "none";
  }

  if (file.kind === "image") {
    return "image";
  }

  if (file.extension.toLowerCase() === "pdf") {
    return "pdf";
  }

  return "none";
}

function statusLabel(status: string) {
  switch (status) {
    case "indexing":
      return "Indexing";
    case "ready":
      return "Ready";
    case "error":
      return "Needs attention";
    default:
      return "Idle";
  }
}

function statusPillClass(status: string) {
  switch (status) {
    case "ready":
      return "inline-flex items-center rounded-full bg-[#e7f6f0] px-3 py-1 text-sm font-medium text-[#24604a]";
    case "indexing":
      return "inline-flex items-center rounded-full bg-[#f7efe0] px-3 py-1 text-sm font-medium text-[#8b6322]";
    case "error":
      return "inline-flex items-center rounded-full bg-[#fff0ec] px-3 py-1 text-sm font-medium text-[#9a4d3a]";
    default:
      return "inline-flex items-center rounded-full bg-[#f0f1f4] px-3 py-1 text-sm font-medium text-[#72787d]";
  }
}

function stagePercent(indexed: number, total: number) {
  if (total <= 0) {
    return 100;
  }

  return Math.round((indexed / total) * 100);
}

function formatPassValue(indexed: number, total: number) {
  if (total <= 0) {
    return "n/a";
  }

  return `${indexed.toLocaleString()} / ${total.toLocaleString()}`;
}

function rootPipelineLabel(root: IndexedRoot, status?: IndexStatus) {
  if (status?.status === "running") {
    return "Scanning metadata";
  }

  if (root.contentPendingCount > 0) {
    return "Extracting content";
  }

  if (root.semanticPendingCount > 0) {
    return "Enriching semantics";
  }

  if (root.fileCount > 0) {
    return "All passes complete";
  }

  return "Queued";
}

function contentStatusLabel(status: string | null) {
  switch (status) {
    case "indexed":
      return "Indexed";
    case "empty":
      return "Empty";
    case "error":
      return "Error";
    case "unsupported":
      return "Unsupported";
    default:
      return "Pending";
  }
}

function semanticStatusLabel(status: string | null) {
  switch (status) {
    case "indexed":
      return "Semantic ready";
    case "empty":
      return "No semantic text";
    case "error":
      return "Semantic error";
    case "unsupported":
      return "Metadata only";
    default:
      return "Semantic pending";
  }
}

function detailStatusLabel(file: FileDetails) {
  if (file.semanticStatus && file.semanticStatus !== "unsupported") {
    return semanticStatusLabel(file.semanticStatus);
  }

  return contentStatusLabel(file.contentStatus);
}

function kindLabel(kind: string) {
  switch (kind) {
    case "document":
      return "Document";
    case "image":
      return "Image";
    case "code":
      return "Code";
    case "text":
      return "Text";
    default:
      return "File";
  }
}

function formatBytes(bytes: number) {
  if (bytes < 1024) {
    return `${bytes} B`;
  }

  const units = ["KB", "MB", "GB", "TB"];
  let value = bytes / 1024;
  let unitIndex = 0;

  while (value >= 1024 && unitIndex < units.length - 1) {
    value /= 1024;
    unitIndex += 1;
  }

  return `${value.toFixed(value >= 100 ? 0 : 1)} ${units[unitIndex]}`;
}

function formatDate(timestamp: number) {
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(new Date(timestamp * 1000));
}

function formatRelativeDate(timestamp: number) {
  const days = Math.max(1, Math.round((Date.now() - timestamp * 1000) / (1000 * 60 * 60 * 24)));
  if (days <= 1) {
    return "Today";
  }
  if (days < 7) {
    return `${days} days ago`;
  }
  return formatDate(timestamp);
}

function formatCompact(value: number) {
  return new Intl.NumberFormat(undefined, {
    notation: "compact",
    maximumFractionDigits: 1,
  }).format(value);
}

function shortPath(path: string) {
  const parts = path.split("/");
  return parts.slice(-2).join("/") || path;
}

function getErrorMessage(error: unknown) {
  if (error instanceof Error) {
    return error.message;
  }

  return String(error);
}

function cx(...parts: Array<string | false | null | undefined>) {
  return parts.filter(Boolean).join(" ");
}

function iconForKind(kind: string) {
  switch (kind) {
    case "image":
      return <ImageIcon />;
    case "code":
      return <CodeIcon />;
    default:
      return <DocumentIcon />;
  }
}

function HomeIcon() {
  return (
    <svg viewBox="0 0 24 24" fill="none" className="h-5 w-5">
      <path d="M4 10.5 12 4l8 6.5v9a1 1 0 0 1-1 1h-5v-6h-4v6H5a1 1 0 0 1-1-1v-9Z" fill="currentColor" />
    </svg>
  );
}

function ResultsIcon() {
  return (
    <svg viewBox="0 0 24 24" fill="none" className="h-5 w-5">
      <rect x="4" y="4" width="16" height="16" rx="3" fill="currentColor" opacity="0.2" />
      <path d="M8 16v-4M12 16V8M16 16v-6" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" />
    </svg>
  );
}

function DatabaseIcon() {
  return (
    <svg viewBox="0 0 24 24" fill="none" className="h-5 w-5">
      <ellipse cx="12" cy="6.5" rx="7" ry="3.5" fill="currentColor" />
      <path d="M5 6.5v5c0 1.93 3.13 3.5 7 3.5s7-1.57 7-3.5v-5M5 11.5v5c0 1.93 3.13 3.5 7 3.5s7-1.57 7-3.5v-5" stroke="currentColor" strokeWidth="1.6" />
    </svg>
  );
}

function SettingsIcon() {
  return (
    <svg viewBox="0 0 24 24" fill="none" className="h-5 w-5">
      <path d="m12 3 2 1 2.3-.3.9 2.1 1.8 1.4-.7 2.2.7 2.2-1.8 1.4-.9 2.1L14 20l-2 1-2-1-2.3.3-.9-2.1-1.8-1.4.7-2.2-.7-2.2 1.8-1.4.9-2.1L10 4l2-1Z" fill="currentColor" opacity="0.2" />
      <circle cx="12" cy="12" r="3" stroke="currentColor" strokeWidth="1.8" />
    </svg>
  );
}

function UserIcon() {
  return (
    <svg viewBox="0 0 24 24" fill="none" className="h-5 w-5">
      <circle cx="12" cy="8" r="4" stroke="currentColor" strokeWidth="1.8" />
      <path d="M5 20a7 7 0 0 1 14 0" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" />
    </svg>
  );
}

function SearchIcon({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 24 24" fill="none" className={className ?? "h-5 w-5"}>
      <circle cx="11" cy="11" r="6.5" stroke="currentColor" strokeWidth="1.8" />
      <path d="m16 16 4 4" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" />
    </svg>
  );
}

function SparkleIcon({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 24 24" fill="none" className={className ?? "h-5 w-5"}>
      <path d="m12 3 1.6 4.4L18 9l-4.4 1.6L12 15l-1.6-4.4L6 9l4.4-1.6L12 3ZM19 14l.9 2.1L22 17l-2.1.9L19 20l-.9-2.1L16 17l2.1-.9L19 14ZM5 14l.9 2.1L8 17l-2.1.9L5 20l-.9-2.1L2 17l2.1-.9L5 14Z" fill="currentColor" />
    </svg>
  );
}

function ArrowRightIcon() {
  return (
    <svg viewBox="0 0 24 24" fill="none" className="h-5 w-5">
      <path d="M5 12h14M13 5l7 7-7 7" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}

function PlusIcon() {
  return (
    <svg viewBox="0 0 24 24" fill="none" className="h-5 w-5">
      <path d="M12 5v14M5 12h14" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" />
    </svg>
  );
}

function FilterIcon() {
  return (
    <svg viewBox="0 0 24 24" fill="none" className="h-5 w-5">
      <path d="M5 7h14M8 12h8M10 17h4" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" />
    </svg>
  );
}

function ShareIcon() {
  return (
    <svg viewBox="0 0 24 24" fill="none" className="h-5 w-5">
      <circle cx="18" cy="5" r="2.5" stroke="currentColor" strokeWidth="1.8" />
      <circle cx="6" cy="12" r="2.5" stroke="currentColor" strokeWidth="1.8" />
      <circle cx="18" cy="19" r="2.5" stroke="currentColor" strokeWidth="1.8" />
      <path d="m8.4 10.8 7.2-4.1M8.4 13.2l7.2 4.1" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" />
    </svg>
  );
}

function RefreshIcon() {
  return (
    <svg viewBox="0 0 24 24" fill="none" className="h-5 w-5">
      <path d="M20 12a8 8 0 1 1-2.3-5.7M20 4v6h-6" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}

function FolderIcon() {
  return (
    <svg viewBox="0 0 24 24" fill="none" className="h-6 w-6">
      <path d="M4 8a2 2 0 0 1 2-2h4l2 2h6a2 2 0 0 1 2 2v7a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V8Z" fill="currentColor" opacity="0.22" />
      <path d="M4 8a2 2 0 0 1 2-2h4l2 2h6a2 2 0 0 1 2 2v7a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V8Z" stroke="currentColor" strokeWidth="1.8" />
    </svg>
  );
}

function DocumentIcon() {
  return (
    <svg viewBox="0 0 24 24" fill="none" className="h-6 w-6">
      <path d="M7 3h7l5 5v11a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2Z" fill="currentColor" opacity="0.2" />
      <path d="M14 3v5h5M9 13h6M9 17h6" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}

function ImageIcon() {
  return (
    <svg viewBox="0 0 24 24" fill="none" className="h-6 w-6">
      <rect x="4" y="5" width="16" height="14" rx="2.5" fill="currentColor" opacity="0.16" />
      <path d="M7 16 11 12l3 3 2-2 2 3M9 9.5h.01" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}

function CodeIcon() {
  return (
    <svg viewBox="0 0 24 24" fill="none" className="h-6 w-6">
      <path d="m9 8-4 4 4 4M15 8l4 4-4 4M13 5l-2 14" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}

function HelpIcon() {
  return (
    <svg viewBox="0 0 24 24" fill="none" className="h-5 w-5">
      <circle cx="12" cy="12" r="9" stroke="currentColor" strokeWidth="1.8" />
      <path d="M9.6 9.3a2.7 2.7 0 1 1 4.7 2c-.7.8-1.8 1.2-1.8 2.7M12 17h.01" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" />
    </svg>
  );
}

function PinIcon() {
  return (
    <svg viewBox="0 0 24 24" fill="none" className="h-5 w-5">
      <path d="m14 4 6 6-3 1-2 7-2-2-2 5-1-8-4-4 8-1Z" fill="currentColor" />
    </svg>
  );
}

function DotsIcon() {
  return (
    <svg viewBox="0 0 24 24" fill="none" className="h-5 w-5">
      <circle cx="6" cy="12" r="1.8" fill="currentColor" />
      <circle cx="12" cy="12" r="1.8" fill="currentColor" />
      <circle cx="18" cy="12" r="1.8" fill="currentColor" />
    </svg>
  );
}

function BookmarkIcon() {
  return (
    <svg viewBox="0 0 24 24" fill="none" className="h-5 w-5">
      <path d="M7 4h10v16l-5-3-5 3V4Z" fill="currentColor" opacity="0.22" />
      <path d="M7 4h10v16l-5-3-5 3V4Z" stroke="currentColor" strokeWidth="1.8" />
    </svg>
  );
}

function ListIcon() {
  return (
    <svg viewBox="0 0 24 24" fill="none" className="h-5 w-5 text-[#737792]">
      <path d="M5 7h14M5 12h14M5 17h14" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" />
    </svg>
  );
}
