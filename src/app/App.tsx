import logoSrc from "../assets/logo.png";
import { panelClass, primaryButtonClass } from "./constants";
import { useAppController } from "./useAppController";
import { SidebarLink } from "../components/navigation";
import {
  DatabaseIcon,
  HomeIcon,
  PlusIcon,
  ResultsIcon,
  SettingsIcon,
  UserIcon,
} from "../components/icons";
import { HomeView } from "../views/HomeView";
import { ResultsView } from "../views/ResultsView";
import { SettingsView } from "../views/SettingsView";
import { SourcesView } from "../views/SourcesView";
import { cx } from "../lib/appHelpers";

export function App() {
  const { state, derived, actions, refs } = useAppController();

  return (
    <div className="min-h-dvh p-4 md:p-6">
      <div className="grid min-h-[calc(100dvh-2rem)] grid-cols-1 gap-4 xl:grid-cols-[290px_minmax(0,1fr)]">
        <aside className={cx(panelClass, "flex flex-col overflow-hidden p-5")}>
          <div className="flex items-start gap-4">
            <img src={logoSrc} alt="Mira" className="h-12 w-12 shrink-0 rounded-2xl object-cover" />
            <div>
              <p className="display-type text-[1.1rem] leading-6 text-[#1d2320]">Mira</p>
              <p className="mt-1 text-sm uppercase tracking-[0.16em] text-[#80858a]">
                AI-powered search
              </p>
            </div>
          </div>

          <button className={cx(primaryButtonClass, "mt-8 w-full")} onClick={() => void actions.handleAddFolder()}>
            <PlusIcon />
            Add source
          </button>

          <nav className="mt-8 space-y-2">
            <SidebarLink
              label="Home"
              active={state.currentView === "home"}
              onClick={() => actions.setCurrentView("home")}
              icon={<HomeIcon />}
            />
            <SidebarLink
              label="Results"
              active={state.currentView === "results"}
              onClick={() => actions.setCurrentView("results")}
              icon={<ResultsIcon />}
              badge={derived.visibleResults.length > 0 ? derived.visibleResults.length : undefined}
            />
            <SidebarLink
              label="Sources"
              active={state.currentView === "sources"}
              onClick={() => actions.setCurrentView("sources")}
              icon={<DatabaseIcon />}
              badge={state.roots.length > 0 ? state.roots.length : undefined}
            />
            <SidebarLink
              label="Settings"
              active={state.currentView === "settings"}
              onClick={() => actions.setCurrentView("settings")}
              icon={<SettingsIcon />}
            />
          </nav>

          <div className="mt-8 rounded-[24px] bg-[#f4f1ec] p-4">
            <p className="text-[0.72rem] uppercase tracking-[0.14em] text-[#7c8187]">
              Current status
            </p>
            <p className="display-type mt-2 text-[1.35rem] leading-8 text-[#222825]">
              {derived.currentStatusText}
            </p>
            <p className="mt-2 text-sm leading-6 text-[#68716d]">
              {derived.runningIndexCount > 0
                ? "Metadata indexing is happening in the background and deeper passes will continue afterward."
                : derived.totalContentPending > 0 || derived.totalSemanticPending > 0
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
                  {state.roots.filter((root) => root.status === "ready").length} active source
                  {state.roots.filter((root) => root.status === "ready").length === 1 ? "" : "s"}
                </p>
              </div>
            </div>
          </div>
        </aside>

        <main className={cx(panelClass, "overflow-hidden")}>
          <div className="flex h-full flex-col">
            <div className="min-h-0 flex-1 overflow-auto px-5 py-6 sm:px-7 sm:py-7">
              {state.currentView === "home" && (
                <HomeView
                  roots={state.roots}
                  totalFiles={derived.totalFiles}
                  runningIndexCount={derived.runningIndexCount}
                  query={state.query}
                  setQuery={actions.setQuery}
                  setCurrentView={actions.setCurrentView}
                  recentSearches={state.recentSearches}
                  savedResults={state.savedResults}
                  selectedFile={state.selectedFile}
                  selectedPreviewUrl={derived.selectedPreviewUrl}
                  pinnedRoots={derived.pinnedRoots}
                  bindPreviewNode={refs.bindHomePreviewNode}
                  onShowSavedResult={actions.handleShowSavedResult}
                  onRemoveSavedResult={actions.removeSavedResult}
                  message={state.message}
                />
              )}

              {state.currentView === "results" && (
                <ResultsView
                  query={state.query}
                  setQuery={actions.setQuery}
                  results={derived.visibleResults}
                  selectedFile={state.selectedFile}
                  selectedPreviewUrl={derived.selectedPreviewUrl}
                  recentSearches={state.recentSearches}
                  resultViewMode={state.resultViewMode}
                  activeKinds={state.activeKinds}
                  isHydrating={state.isHydrating}
                  isSearching={state.isSearching}
                  savedResultPaths={derived.savedResultPaths}
                  onQuerySelect={actions.setQuery}
                  onSetViewMode={actions.setResultViewMode}
                  onToggleKindFilter={actions.toggleKindFilter}
                  onClearKindFilters={actions.clearKindFilters}
                  onSelectResult={actions.showSearchPreview}
                  onToggleSavedResult={actions.toggleSavedResult}
                  onOpenFile={actions.handleOpenFile}
                  onRevealFile={actions.handleRevealFile}
                  message={state.message}
                />
              )}

              {state.currentView === "sources" && (
                <SourcesView
                  roots={state.roots}
                  statuses={state.statuses}
                  totalFiles={derived.totalFiles}
                  totalContentIndexed={derived.totalContentIndexed}
                  totalContentPending={derived.totalContentPending}
                  totalSemanticIndexed={derived.totalSemanticIndexed}
                  totalSemanticPending={derived.totalSemanticPending}
                  runningIndexCount={derived.runningIndexCount}
                  selectedRootIds={state.selectedRootIds}
                  toggleRoot={actions.toggleRoot}
                  onAddFolder={actions.handleAddFolder}
                  onRescan={actions.handleRescan}
                  onRescanAll={actions.handleRescanAll}
                  onRemoveRoot={actions.handleRemoveRoot}
                />
              )}

              {state.currentView === "settings" && (
                <SettingsView
                  headerTitle={derived.headerTitle}
                  totalFiles={derived.totalFiles}
                  roots={state.roots}
                  currentStatusText={derived.currentStatusText}
                />
              )}
            </div>
          </div>
        </main>
      </div>
    </div>
  );
}
