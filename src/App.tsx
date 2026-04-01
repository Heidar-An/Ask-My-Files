import { useEffect, useMemo, useState } from "react";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { open as openDialog } from "@tauri-apps/plugin-dialog";

type IndexedRoot = {
  id: number;
  path: string;
  status: string;
  fileCount: number;
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
  matchReasons: string[];
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
};

type SearchRequest = {
  query: string;
  rootIds?: number[];
  limit?: number;
};

const SEARCH_PLACEHOLDERS = [
  "passport photo",
  "quarterly budget",
  "typescript config",
  "contract draft",
];

const panelClass =
  "rounded-[28px] border border-[color:var(--panel-border)] bg-[color:var(--panel)] shadow-[var(--shadow)] backdrop-blur-xl";
const subtlePanelClass =
  "rounded-[22px] border border-[color:var(--panel-border)] bg-white/75 shadow-[0_16px_24px_rgba(34,67,57,0.10)] backdrop-blur-xl";
const baseButtonClass =
  "rounded-full border border-black/10 bg-white/85 px-4 py-2.5 text-sm font-medium text-[#14211d] transition hover:-translate-y-0.5 hover:border-black/20";

export function App() {
  const [roots, setRoots] = useState<IndexedRoot[]>([]);
  const [statuses, setStatuses] = useState<Record<number, IndexStatus>>({});
  const [results, setResults] = useState<SearchResult[]>([]);
  const [selectedRootIds, setSelectedRootIds] = useState<number[]>([]);
  const [query, setQuery] = useState("");
  const [selectedFileId, setSelectedFileId] = useState<number | null>(null);
  const [selectedFile, setSelectedFile] = useState<FileDetails | null>(null);
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
    const selected = await openDialog({
      directory: true,
      multiple: false,
      title: "Choose a folder to index",
    });

    if (!selected || Array.isArray(selected)) {
      return;
    }

    try {
      const root = await invoke<IndexedRoot>("add_index_root", { path: selected });
      await invoke("start_index", { rootId: root.id });
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

  const selectedPreviewUrl =
    selectedFile?.previewPath ? convertFileSrc(selectedFile.previewPath) : null;
  const activeRootCount = (filteredRootIds?.length ?? roots.length) || 0;

  return (
    <div className="min-h-dvh p-4 md:p-6">
      <div className="grid min-h-[calc(100dvh-2rem)] grid-cols-1 gap-4 xl:grid-cols-[320px_minmax(0,1fr)_360px]">
        <aside className={cx(panelClass, "flex flex-col p-5")}>
          <div className="flex flex-col gap-4 sm:flex-row sm:items-start sm:justify-between xl:flex-col xl:gap-5 2xl:flex-row">
            <div>
              <p className="mb-1 text-[0.72rem] uppercase tracking-[0.14em] text-[color:var(--text-soft)]">
                Ask My Files
              </p>
              <h1 className="display-type text-[1.7rem] leading-none text-[#17211d]">
                Indexed folders
              </h1>
            </div>
            <button
              className={cx(
                baseButtonClass,
                "border-transparent bg-[linear-gradient(135deg,var(--accent)_0%,#2c7a61_100%)] text-[#f7fbf8]",
              )}
              onClick={() => void handleAddFolder()}
            >
              Add folder
            </button>
          </div>

          <div className="mt-5 grid grid-cols-2 gap-3 rounded-[18px] bg-white/55 p-4">
            <Metric label="roots" value={roots.length.toLocaleString()} />
            <Metric
              label="files"
              value={roots.reduce((total, root) => total + root.fileCount, 0).toLocaleString()}
            />
          </div>

          <button
            className={cx(
              "mt-4 flex w-full items-center justify-between rounded-[18px] border px-4 py-3 text-left text-sm font-medium transition",
              selectedRootIds.length === 0
                ? "border-[rgba(35,95,75,0.26)] bg-white/95 shadow-[0_16px_24px_rgba(34,67,57,0.10)]"
                : "border-transparent bg-white/55 hover:border-black/10",
            )}
            onClick={() => setSelectedRootIds([])}
          >
            <span>All folders</span>
            <span className="text-[color:var(--text-soft)]">{roots.length}</span>
          </button>

          <div className="mt-3 flex min-h-0 flex-1 flex-col gap-3 overflow-auto pr-1">
            {roots.map((root) => {
              const status = statuses[root.id];
              const isSelected = selectedRootIds.includes(root.id);
              const progress =
                status && status.total > 0
                  ? Math.round((status.processed / status.total) * 100)
                  : null;

              return (
                <article
                  key={root.id}
                  className={cx(
                    subtlePanelClass,
                    "p-3.5",
                    isSelected && "border-[rgba(35,95,75,0.26)] bg-white/95",
                  )}
                >
                  <button className="w-full text-left" onClick={() => toggleRoot(root.id)}>
                    <span className="block break-words text-sm font-semibold text-[#17211d]">
                      {root.path}
                    </span>
                    <span className="mt-1 inline-block text-sm text-[color:var(--text-soft)]">
                      {root.fileCount.toLocaleString()} files
                    </span>
                  </button>

                  <div className="mt-3 flex flex-wrap items-center gap-2.5 text-sm text-[color:var(--text-soft)]">
                    <span className={statusClass(root.status)}>{statusLabel(root.status)}</span>
                    <span>
                      {root.lastIndexedAt ? formatDate(root.lastIndexedAt) : "Not indexed yet"}
                    </span>
                  </div>

                  {status && status.status === "running" ? (
                    <div className="mt-3">
                      <div className="h-2 w-full overflow-hidden rounded-full bg-[rgba(35,95,75,0.10)]">
                        <span
                          className="block h-full rounded-full bg-[linear-gradient(90deg,#dcb87b_0%,#4b8c74_100%)]"
                          style={{ width: `${progress ?? 6}%` }}
                        />
                      </div>
                      <p className="mt-2 text-xs text-[color:var(--text-soft)]">
                        {status.processed.toLocaleString()}
                        {status.total > 0 ? ` / ${status.total.toLocaleString()}` : ""} files
                      </p>
                    </div>
                  ) : null}

                  {root.lastError ? (
                    <p className="mt-3 text-sm text-[color:var(--danger)]">{root.lastError}</p>
                  ) : null}

                  <div className="mt-4 flex flex-wrap gap-2.5">
                    <button className={baseButtonClass} onClick={() => void handleRescan(root.id)}>
                      Rescan
                    </button>
                    <button
                      className={cx(baseButtonClass, "text-[color:var(--danger)]")}
                      onClick={() => void handleRemoveRoot(root.id)}
                    >
                      Remove
                    </button>
                  </div>
                </article>
              );
            })}

            {roots.length === 0 ? (
              <div className={cx(subtlePanelClass, "p-4 text-sm text-[color:var(--text-soft)]")}>
                <p>Add your first folder to start building a local file index.</p>
              </div>
            ) : null}
          </div>
        </aside>

        <main className="flex min-h-0 flex-col gap-4">
          <section className={cx(panelClass, "p-5 sm:p-6")}>
            <div className="flex flex-col gap-4 xl:flex-row xl:items-start xl:justify-between">
              <div>
                <p className="mb-1 text-[0.72rem] uppercase tracking-[0.14em] text-[color:var(--text-soft)]">
                  Natural file lookup
                </p>
                <h2 className="display-type max-w-[12ch] text-[2rem] leading-[1.02] text-[#17211d]">
                  Find the right file without remembering the exact name
                </h2>
              </div>

              <div className="flex flex-wrap items-center gap-2.5">
                {runningIndexCount > 0 ? (
                  <span className={statusClass("indexing")}>
                    {runningIndexCount} indexing
                  </span>
                ) : null}
                {isSearching ? <span className={statusClass("searching")}>Searching</span> : null}
              </div>
            </div>

            <label className="mt-6 block">
              <span className="text-sm text-[color:var(--text-soft)]">Search your files</span>
              <input
                className="mt-2 w-full rounded-[24px] border border-black/10 bg-[color:var(--panel-strong)] px-5 py-5 text-[1.05rem] text-[#17211d] shadow-[inset_0_1px_0_rgba(255,255,255,0.65)] outline-none transition focus:border-[rgba(35,95,75,0.22)] focus:ring-4 focus:ring-[rgba(35,95,75,0.12)]"
                value={query}
                onChange={(event) => setQuery(event.target.value)}
                placeholder={
                  SEARCH_PLACEHOLDERS[Math.floor(Date.now() / 1000) % SEARCH_PLACEHOLDERS.length]
                }
              />
            </label>

            <div className="mt-4 flex flex-col gap-2 text-sm text-[color:var(--text-soft)] lg:flex-row lg:items-center lg:justify-between">
              <p>
                {query.trim().length === 0
                  ? "Showing recent files from the indexed folders."
                  : `Searching ${activeRootCount} folder views for “${query}”.`}
              </p>
              {message ? <p className="text-[color:var(--danger)]">{message}</p> : null}
            </div>
          </section>

          <section className={cx(panelClass, "flex min-h-0 flex-1 flex-col p-5 sm:p-6")}>
            <header className="flex flex-col gap-2 text-[color:var(--text-soft)] sm:flex-row sm:items-center sm:justify-between">
              <h3 className="display-type text-[1.15rem] text-[#17211d]">
                {query.trim().length === 0 ? "Recent files" : "Results"}
              </h3>
              <span className="text-sm">{results.length.toLocaleString()} shown</span>
            </header>

            {isHydrating ? (
              <div className={cx(subtlePanelClass, "mt-4 p-4 text-sm text-[color:var(--text-soft)]")}>
                <p>Loading your workspace…</p>
              </div>
            ) : null}

            {!isHydrating && results.length === 0 ? (
              <div className={cx(subtlePanelClass, "mt-4 p-4 text-sm text-[color:var(--text-soft)]")}>
                <p>
                  {roots.length === 0
                    ? "Add a folder to start searching."
                    : "No files matched that query yet."}
                </p>
              </div>
            ) : null}

            <div className="mt-4 flex min-h-0 flex-1 flex-col gap-3 overflow-auto pr-1">
              {results.map((result) => (
                <button
                  key={result.fileId}
                  className={cx(
                    subtlePanelClass,
                    "w-full p-4 text-left transition hover:-translate-y-0.5",
                    result.fileId === selectedFileId &&
                      "border-[rgba(35,95,75,0.26)] bg-white/95",
                  )}
                  onClick={() => setSelectedFileId(result.fileId)}
                >
                  <div className="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
                    <div className="min-w-0">
                      <p className="display-type text-[1.1rem] leading-snug text-[#17211d]">
                        {result.name}
                      </p>
                      <p className="mt-1 break-words text-sm text-[color:var(--text-soft)]">
                        {result.path}
                      </p>
                    </div>
                    <span className={kindClass(result.kind)}>{result.kind}</span>
                  </div>

                  <div className="mt-3 flex flex-wrap items-center gap-2.5 text-xs text-[color:var(--text-soft)] sm:text-sm">
                    <span>{formatBytes(result.size)}</span>
                    <span>
                      {result.modifiedAt ? formatDate(result.modifiedAt) : "Unknown date"}
                    </span>
                    {result.score > 0 ? <span>score {result.score}</span> : null}
                  </div>

                  <div className="mt-3 flex flex-wrap gap-2">
                    {result.matchReasons.map((reason) => (
                      <span
                        key={reason}
                        className="inline-flex items-center rounded-full bg-[color:var(--accent-wash)] px-2.5 py-1 text-xs font-medium text-[color:var(--accent-strong)]"
                      >
                        {reason}
                      </span>
                    ))}
                  </div>
                </button>
              ))}
            </div>
          </section>
        </main>

        <aside className={cx(panelClass, "flex flex-col p-5")}>
          <header className="flex flex-col gap-2 text-[color:var(--text-soft)] sm:flex-row sm:items-center sm:justify-between">
            <h3 className="display-type text-[1.15rem] text-[#17211d]">Preview</h3>
            {selectedFile ? <span className="text-sm">{selectedFile.kind}</span> : null}
          </header>

          {selectedFile ? (
            <div className="mt-4 flex flex-1 flex-col gap-4">
              {selectedPreviewUrl ? (
                <div className="grid min-h-[240px] place-items-center rounded-[24px] border border-black/10 bg-[linear-gradient(135deg,rgba(255,255,255,0.94),rgba(233,239,235,0.92))] p-3">
                  <img
                    src={selectedPreviewUrl}
                    alt={selectedFile.name}
                    className="max-h-[320px] w-full rounded-[22px] object-contain"
                  />
                </div>
              ) : (
                <div className="grid min-h-[240px] place-items-center rounded-[24px] border border-black/10 bg-[linear-gradient(135deg,rgba(255,255,255,0.94),rgba(233,239,235,0.92))]">
                  <span className="rounded-full bg-[color:var(--accent-wash)] px-4 py-2 text-xs uppercase tracking-[0.14em] text-[color:var(--accent-strong)]">
                    {selectedFile.extension || "file"}
                  </span>
                </div>
              )}

              <div className="space-y-4">
                <div>
                  <p className="mb-1 text-[0.72rem] uppercase tracking-[0.14em] text-[color:var(--text-soft)]">
                    Selected file
                  </p>
                  <h3 className="display-type text-[1.3rem] leading-tight text-[#17211d]">
                    {selectedFile.name}
                  </h3>
                </div>

                <dl className="grid gap-3.5 text-sm">
                  <MetadataRow label="Location" value={selectedFile.path} />
                  <MetadataRow label="Root" value={selectedFile.rootPath} />
                  <MetadataRow label="Size" value={formatBytes(selectedFile.size)} />
                  <MetadataRow
                    label="Modified"
                    value={
                      selectedFile.modifiedAt
                        ? formatDate(selectedFile.modifiedAt)
                        : "Unknown"
                    }
                  />
                </dl>

                <div className="flex flex-wrap gap-2.5">
                  <button
                    className={cx(
                      baseButtonClass,
                      "border-transparent bg-[linear-gradient(135deg,var(--accent)_0%,#2c7a61_100%)] text-[#f7fbf8]",
                    )}
                    onClick={() => void handleOpenFile(selectedFile.path)}
                  >
                    Open file
                  </button>
                  <button
                    className={baseButtonClass}
                    onClick={() => void handleRevealFile(selectedFile.path)}
                  >
                    Reveal in Finder
                  </button>
                </div>
              </div>
            </div>
          ) : (
            <div className={cx(subtlePanelClass, "mt-4 p-4 text-sm text-[color:var(--text-soft)]")}>
              <p>Select a result to inspect metadata and quick actions.</p>
            </div>
          )}
        </aside>
      </div>
    </div>
  );
}

function Metric({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <span className="display-type block text-[1.75rem] leading-none text-[#17211d]">
        {value}
      </span>
      <span className="mt-1 block text-sm text-[color:var(--text-soft)]">{label}</span>
    </div>
  );
}

function MetadataRow({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <dt className="text-[0.72rem] uppercase tracking-[0.1em] text-[color:var(--text-soft)]">
        {label}
      </dt>
      <dd className="mt-1 break-words text-[#17211d]">{value}</dd>
    </div>
  );
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

function statusClass(status: string) {
  switch (status) {
    case "ready":
      return "inline-flex items-center rounded-full bg-[rgba(68,152,111,0.14)] px-2.5 py-1 text-xs font-medium text-[#20523f]";
    case "indexing":
      return "inline-flex items-center rounded-full bg-[rgba(179,132,59,0.15)] px-2.5 py-1 text-xs font-medium text-[#855c1a]";
    case "error":
      return "inline-flex items-center rounded-full bg-[rgba(178,84,59,0.14)] px-2.5 py-1 text-xs font-medium text-[#8d402c]";
    case "searching":
      return "inline-flex items-center rounded-full bg-white/75 px-2.5 py-1 text-xs font-medium text-[color:var(--text-soft)]";
    default:
      return "inline-flex items-center rounded-full bg-white/75 px-2.5 py-1 text-xs font-medium text-[color:var(--text-soft)]";
  }
}

function kindClass(kind: string) {
  switch (kind) {
    case "image":
      return "inline-flex items-center rounded-full bg-[rgba(63,116,167,0.14)] px-2.5 py-1 text-xs font-medium text-[#2c5378]";
    case "document":
    case "text":
    case "code":
      return "inline-flex items-center rounded-full bg-[rgba(35,95,75,0.12)] px-2.5 py-1 text-xs font-medium text-[color:var(--accent-strong)]";
    default:
      return "inline-flex items-center rounded-full bg-white/75 px-2.5 py-1 text-xs font-medium text-[color:var(--text-soft)]";
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

function getErrorMessage(error: unknown) {
  if (error instanceof Error) {
    return error.message;
  }

  return String(error);
}

function cx(...parts: Array<string | false | null | undefined>) {
  return parts.filter(Boolean).join(" ");
}
