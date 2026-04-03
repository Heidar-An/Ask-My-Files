import { FILE_TYPE_FILTERS, buttonClass, panelClass, primaryButtonClass } from "../app/constants";
import type { FileDetails, ResultViewMode, SavedResult, SearchResult } from "../app/types";
import { InfoRow, StatusNotice } from "../components/cards";
import {
  BookmarkIcon,
  DocumentIcon,
  FilterIcon,
  GridIcon,
  ListIcon,
  SearchIcon,
  SparkleIcon,
} from "../components/icons";
import { SelectedFilePreview } from "../components/preview";
import {
  HighlightedSnippet,
  PreviewInsightCard,
  ResultGridCard,
  ResultListRow,
} from "../components/results";
import {
  cx,
  detailStatusLabel,
  formatBytes,
  formatDate,
  kindLabel,
} from "../lib/appHelpers";

export interface ResultsViewProps {
  query: string;
  setQuery: (value: string) => void;
  results: SearchResult[];
  selectedFile: FileDetails | null;
  selectedPreviewUrl: string | null;
  recentSearches: string[];
  resultViewMode: ResultViewMode;
  activeKinds: string[];
  isHydrating: boolean;
  isSearching: boolean;
  savedResultPaths: Set<string>;
  onQuerySelect: (query: string) => void;
  onSetViewMode: (mode: ResultViewMode) => void;
  onToggleKindFilter: (kind: string) => void;
  onClearKindFilters: () => void;
  onSelectResult: (fileId: number) => void;
  onToggleSavedResult: (
    result: Pick<SavedResult, "path" | "name" | "kind" | "extension" | "modifiedAt">,
  ) => void;
  onOpenFile: (path: string) => Promise<void>;
  onRevealFile: (path: string) => Promise<void>;
  message: string | null;
}

export function ResultsView({
  query,
  setQuery,
  results,
  selectedFile,
  selectedPreviewUrl,
  recentSearches,
  resultViewMode,
  activeKinds,
  isHydrating,
  isSearching,
  savedResultPaths,
  onQuerySelect,
  onSetViewMode,
  onToggleKindFilter,
  onClearKindFilters,
  onSelectResult,
  onToggleSavedResult,
  onOpenFile,
  onRevealFile,
  message,
}: ResultsViewProps) {
  const emptyState = isHydrating
    ? "Loading your workspace..."
    : query.trim().length === 0
      ? "Start with a query like “passport photo” or “tax return 2024”."
      : "No files matched that query yet.";
  const selectedFileSaved = selectedFile ? savedResultPaths.has(selectedFile.path) : false;

  return (
    <div className="space-y-6">
      <section className="px-1 pt-2">
        <div className="mx-auto mb-6 max-w-5xl rounded-[28px] border border-black/5 bg-white/78 p-3 shadow-[0_22px_60px_rgba(85,93,122,0.08)]">
          <div className="flex items-center gap-4 rounded-[22px] bg-[#fcfbf8] px-5 py-4">
            <SparkleIcon className="h-6 w-6 shrink-0 text-[#737792]" />
            <input
              className="w-full bg-transparent text-[1.15rem] text-[#222825] outline-none placeholder:text-[#9da2a6]"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              autoComplete="off"
              autoCorrect="off"
              autoCapitalize="none"
              spellCheck={false}
              placeholder="Search your workspace..."
              // eslint-disable-next-line jsx-a11y/no-autofocus
              autoFocus
            />
          </div>
        </div>
        <div className="flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between">
          <div>
            <div className="flex flex-wrap items-center gap-3 text-[1.05rem] text-[#686f6c]">
              <span className="inline-flex items-center gap-2 text-[#272d2a]">
                <SparkleIcon className="h-5 w-5 text-[#737792]" />
                Hybrid lexical + visual matches
              </span>
              <span className="rounded-full bg-[#eaecf8] px-3 py-1 text-[0.72rem] uppercase tracking-[0.14em] text-[#5d647f]">
                {results.length.toLocaleString()} result{results.length === 1 ? "" : "s"}
              </span>
              {isSearching ? (
                <span className="rounded-full bg-[#f0f1f4] px-3 py-1 text-[0.72rem] uppercase tracking-[0.14em] text-[#7b8186]">
                  Searching
                </span>
              ) : null}
            </div>
          </div>

          <div className="flex flex-wrap gap-3">
            <div className="inline-flex rounded-[18px] border border-black/8 bg-white/80 p-1">
              <button
                type="button"
                className={cx(
                  "rounded-[14px] px-3 py-2 text-sm font-medium transition",
                  resultViewMode === "list" ? "bg-[#737792] text-white" : "text-[#58605c]",
                )}
                onClick={() => onSetViewMode("list")}
              >
                List
              </button>
              <button
                type="button"
                className={cx(
                  "rounded-[14px] px-3 py-2 text-sm font-medium transition",
                  resultViewMode === "grid" ? "bg-[#737792] text-white" : "text-[#58605c]",
                )}
                onClick={() => onSetViewMode("grid")}
              >
                Grid
              </button>
            </div>
            {activeKinds.length > 0 ? (
              <button type="button" className={buttonClass} onClick={onClearKindFilters}>
                <FilterIcon />
                Clear filters
              </button>
            ) : null}
          </div>
        </div>

        <div className="mt-5 flex flex-wrap gap-2">
          {FILE_TYPE_FILTERS.map((kind) => {
            const active = activeKinds.includes(kind);
            return (
              <button
                key={kind}
                type="button"
                className={cx(
                  "rounded-full px-4 py-2 text-sm font-medium transition",
                  active
                    ? "bg-[#737792] text-white shadow-[0_10px_24px_rgba(115,119,146,0.18)]"
                    : "bg-[#f3f1ea] text-[#646b67] hover:bg-[#ebe7de]",
                )}
                onClick={() => onToggleKindFilter(kind)}
              >
                {kindLabel(kind)}
              </button>
            );
          })}
        </div>

        {recentSearches.length > 0 ? (
          <div className="mt-4 flex flex-wrap items-center gap-2 text-sm text-[#6c7370]">
            <span className="uppercase tracking-[0.16em] text-[0.72rem] text-[#848a8e]">
              Recent
            </span>
            {recentSearches.slice(0, 4).map((recent) => (
              <button
                key={recent}
                type="button"
                className="rounded-full border border-black/5 bg-white px-3 py-1.5 text-[#4f5652] transition hover:bg-[#f8f6f1]"
                onClick={() => onQuerySelect(recent)}
              >
                {recent}
              </button>
            ))}
          </div>
        ) : null}
      </section>

      {results.length === 0 ? (
        <div className={cx(panelClass, "p-8 text-center")}>
          <div className="mx-auto grid h-16 w-16 place-items-center rounded-full bg-[#eef0f6] text-[#737792]">
            {isSearching || isHydrating ? <SearchIcon className="h-6 w-6" /> : <DocumentIcon />}
          </div>
          <p className="display-type mt-5 text-[2rem] text-[#262d2a]">{emptyState}</p>
          <p className="mx-auto mt-3 max-w-2xl text-sm leading-7 text-[#6d7470]">
            {activeKinds.length > 0
              ? "Try removing a type filter or switching back to all results."
              : "You can search by filename, text contents, semantic meaning, and visual similarity."}
          </p>
          {message ? <p className="mt-3 text-sm text-[color:var(--danger)]">{message}</p> : null}
        </div>
      ) : (
        <section className="grid gap-4 xl:grid-cols-[minmax(0,1.1fr)_minmax(320px,0.85fr)]">
          <article className={cx(panelClass, "min-h-0 p-5 sm:p-6")}>
            <div className="flex items-center justify-between gap-3">
              <div className="flex items-center gap-3">
                {resultViewMode === "grid" ? <GridIcon /> : <ListIcon />}
                <div>
                  <h2 className="display-type text-[1.55rem] text-[#202724]">Results</h2>
                  <p className="text-sm text-[#727977]">
                    {resultViewMode === "grid"
                      ? "Compact cards with inline previews."
                      : "Filename-first rows with preview details on the right."}
                  </p>
                </div>
              </div>
              <p className="text-sm text-[#727977]">
                {results.length.toLocaleString()} shown
              </p>
            </div>

            <div
              className={cx(
                "mt-5",
                resultViewMode === "grid"
                  ? "grid gap-3 sm:grid-cols-2 2xl:grid-cols-3"
                  : "space-y-2.5",
              )}
            >
              {results.map((result) =>
                resultViewMode === "grid" ? (
                  <ResultGridCard
                    key={result.fileId}
                    result={result}
                    query={query}
                    selected={selectedFile?.fileId === result.fileId}
                    onSelectResult={onSelectResult}
                    onOpenPreview={onOpenFile}
                  />
                ) : (
                  <ResultListRow
                    key={result.fileId}
                    result={result}
                    query={query}
                    selected={selectedFile?.fileId === result.fileId}
                    onSelectResult={onSelectResult}
                  />
                ),
              )}
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
                  query={query}
                  className="h-60 rounded-[24px]"
                />

                <div>
                  <div className="flex flex-wrap items-start justify-between gap-3">
                    <div className="min-w-0 flex-1">
                      <p className="display-type text-[1.8rem] leading-tight text-[#202724]">
                        {selectedFile.name}
                      </p>
                      <p className="mt-2 text-[0.72rem] uppercase tracking-[0.14em] text-[#7b8186]">
                        {kindLabel(selectedFile.kind)} •{" "}
                        {selectedFile.modifiedAt
                          ? formatDate(selectedFile.modifiedAt)
                          : "Unknown date"}
                      </p>
                    </div>

                    <button
                      type="button"
                      className={cx(
                        buttonClass,
                        selectedFileSaved && "border-transparent bg-[#eff0f8] text-[#5d647f]",
                      )}
                      onClick={() =>
                        onToggleSavedResult({
                          path: selectedFile.path,
                          name: selectedFile.name,
                          kind: selectedFile.kind,
                          extension: selectedFile.extension,
                          modifiedAt: selectedFile.modifiedAt,
                        })
                      }
                    >
                      <BookmarkIcon />
                      {selectedFileSaved ? "Saved" : "Save result"}
                    </button>
                  </div>

                  <HighlightedSnippet
                    className="mt-3 text-sm leading-7 text-[#666d6a]"
                    text={
                      selectedFile.contentSnippet ??
                      selectedFile.semanticSummary ??
                      "This file is available in your workspace. Use the actions below to open or reveal it."
                    }
                    query={query}
                  />
                </div>

                <PreviewInsightCard file={selectedFile} query={query} />

                <div className="grid gap-3 text-sm text-[#666d6a]">
                  <InfoRow label="Location" value={selectedFile.path} />
                  <InfoRow label="Root" value={selectedFile.rootPath} />
                  <InfoRow label="Size" value={formatBytes(selectedFile.size)} />
                  <InfoRow
                    label="Modified"
                    value={selectedFile.modifiedAt ? formatDate(selectedFile.modifiedAt) : "Unknown"}
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
                  <InfoRow label="Indexed" value={formatDate(selectedFile.indexedAt)} />
                </div>

                {selectedFile.extractionError ? (
                  <StatusNotice
                    tone="danger"
                    title="Content extraction needs attention"
                    body={selectedFile.extractionError}
                  />
                ) : null}
                {selectedFile.semanticError ? (
                  <StatusNotice
                    tone="warning"
                    title="Semantic enrichment is partial"
                    body={selectedFile.semanticError}
                  />
                ) : null}

                <div className="flex flex-wrap gap-3">
                  <button
                    type="button"
                    className={primaryButtonClass}
                    onClick={() => void onOpenFile(selectedFile.path)}
                  >
                    Open file
                  </button>
                  <button
                    type="button"
                    className={buttonClass}
                    onClick={() => void onRevealFile(selectedFile.path)}
                  >
                    Reveal in Finder
                  </button>
                </div>
              </div>
            ) : (
              <div className="mt-5 rounded-[24px] bg-[#faf9f6] p-5 text-sm leading-7 text-[#666d6a]">
                Select a result to inspect the file, preview extracted text, and review metadata.
              </div>
            )}

            {message ? <p className="mt-4 text-sm text-[color:var(--danger)]">{message}</p> : null}
          </aside>
        </section>
      )}
    </div>
  );
}
