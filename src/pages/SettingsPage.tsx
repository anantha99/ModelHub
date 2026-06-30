import { useEffect, useState } from "react";
import { getResolvedPaths, getSettings, updateSettings } from "../api/tauri";
import type {
  AppSettings,
  AppSettingsPatch,
  PathIssue,
  ResolvedPath,
  ResolvedPaths,
} from "../api/types";

type SettingsDraft = {
  hfCachePath: string;
  lmStudioModelsPath: string;
  customModelFolders: string[];
  minimizeToTray: boolean;
  enableSymlinkAttempt: boolean;
  scanOnStartup: boolean;
  deleteUsesRecycleBin: boolean;
};

function settingsToDraft(settings: AppSettings): SettingsDraft {
  return {
    hfCachePath: settings.hfCachePath ?? "",
    lmStudioModelsPath: settings.lmStudioModelsPath ?? "",
    customModelFolders: settings.customModelFolders,
    minimizeToTray: settings.minimizeToTray,
    enableSymlinkAttempt: settings.enableSymlinkAttempt,
    scanOnStartup: settings.scanOnStartup,
    deleteUsesRecycleBin: settings.deleteUsesRecycleBin,
  };
}

function draftToPatch(draft: SettingsDraft): AppSettingsPatch {
  return {
    hfCachePath: optionalPath(draft.hfCachePath),
    lmStudioModelsPath: optionalPath(draft.lmStudioModelsPath),
    customModelFolders: draft.customModelFolders
      .map((folder) => folder.trim())
      .filter(Boolean),
    minimizeToTray: draft.minimizeToTray,
    enableSymlinkAttempt: draft.enableSymlinkAttempt,
    scanOnStartup: draft.scanOnStartup,
    deleteUsesRecycleBin: draft.deleteUsesRecycleBin,
  };
}

function optionalPath(value: string): string | null {
  const trimmed = value.trim();
  return trimmed.length > 0 ? trimmed : null;
}

function draftsMatchSettings(draft: SettingsDraft, settings: AppSettings): boolean {
  const currentDraft = settingsToDraft(settings);
  return JSON.stringify(draft) === JSON.stringify(currentDraft);
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

async function fetchSettingsState(): Promise<{
  loadedSettings: AppSettings;
  loadedPaths: ResolvedPaths;
}> {
  const [loadedSettings, loadedPaths] = await Promise.all([
    getSettings(),
    getResolvedPaths(),
  ]);

  return { loadedSettings, loadedPaths };
}

export function SettingsPage() {
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [draft, setDraft] = useState<SettingsDraft | null>(null);
  const [resolvedPaths, setResolvedPaths] = useState<ResolvedPaths | null>(null);
  const [newCustomFolder, setNewCustomFolder] = useState("");
  const [isLoading, setIsLoading] = useState(true);
  const [isSaving, setIsSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [statusMessage, setStatusMessage] = useState<string | null>(null);

  async function loadSettings() {
    setIsLoading(true);
    setError(null);

    try {
      const { loadedSettings, loadedPaths } = await fetchSettingsState();

      setSettings(loadedSettings);
      setDraft(settingsToDraft(loadedSettings));
      setResolvedPaths(loadedPaths);
    } catch (loadError) {
      setError(errorMessage(loadError));
    } finally {
      setIsLoading(false);
    }
  }

  useEffect(() => {
    let isMounted = true;

    async function loadInitialSettings() {
      setIsLoading(true);
      setError(null);

      try {
        const { loadedSettings, loadedPaths } = await fetchSettingsState();

        if (!isMounted) {
          return;
        }

        setSettings(loadedSettings);
        setDraft(settingsToDraft(loadedSettings));
        setResolvedPaths(loadedPaths);
      } catch (loadError) {
        if (isMounted) {
          setError(errorMessage(loadError));
        }
      } finally {
        if (isMounted) {
          setIsLoading(false);
        }
      }
    }

    void loadInitialSettings();

    return () => {
      isMounted = false;
    };
  }, []);

  const hasUnsavedChanges = Boolean(
    draft && settings && !draftsMatchSettings(draft, settings),
  );
  const pathIssueCount = resolvedPaths ? countPathIssues(resolvedPaths) : 0;

  function updateDraft(patch: Partial<SettingsDraft>) {
    setDraft((current) => (current ? { ...current, ...patch } : current));
    setStatusMessage(null);
  }

  function updateCustomFolder(index: number, value: string) {
    setDraft((current) => {
      if (!current) {
        return current;
      }

      return {
        ...current,
        customModelFolders: current.customModelFolders.map((folder, folderIndex) =>
          folderIndex === index ? value : folder,
        ),
      };
    });
    setStatusMessage(null);
  }

  function removeCustomFolder(index: number) {
    setDraft((current) => {
      if (!current) {
        return current;
      }

      return {
        ...current,
        customModelFolders: current.customModelFolders.filter(
          (_folder, folderIndex) => folderIndex !== index,
        ),
      };
    });
    setStatusMessage(null);
  }

  function addCustomFolder() {
    const folder = newCustomFolder.trim();

    if (!folder) {
      return;
    }

    setDraft((current) => {
      if (!current) {
        return current;
      }

      return {
        ...current,
        customModelFolders: [...current.customModelFolders, folder],
      };
    });
    setNewCustomFolder("");
    setStatusMessage(null);
  }

  async function saveSettings() {
    if (!draft) {
      return;
    }

    setIsSaving(true);
    setError(null);
    setStatusMessage(null);

    try {
      const updatedSettings = await updateSettings(draftToPatch(draft));
      const updatedPaths = await getResolvedPaths();

      setSettings(updatedSettings);
      setDraft(settingsToDraft(updatedSettings));
      setResolvedPaths(updatedPaths);
      setStatusMessage("Settings saved. New paths apply to the next scan or download.");
    } catch (saveError) {
      setError(errorMessage(saveError));
    } finally {
      setIsSaving(false);
    }
  }

  function revertDraft() {
    if (settings) {
      setDraft(settingsToDraft(settings));
      setStatusMessage("Unsaved changes reverted.");
    }
  }

  return (
    <div className="page-stack settings-page">
      <header className="page-header settings-hero">
        <div>
          <p className="eyebrow">Settings</p>
          <h2>Paths and app behavior</h2>
          <p>
            Configure where ModelHub scans and writes model files. Secrets stay
            out of the settings file.
          </p>
        </div>
        <div className="settings-hero-actions">
          <button className="secondary-button" onClick={() => void loadSettings()} type="button">
            Refresh
          </button>
          <button
            className="secondary-button"
            disabled={!hasUnsavedChanges || isSaving}
            onClick={revertDraft}
            type="button"
          >
            Revert
          </button>
          <button
            className="primary-button"
            disabled={!hasUnsavedChanges || isSaving || !draft}
            onClick={() => void saveSettings()}
            type="button"
          >
            {isSaving ? "Saving..." : "Save Changes"}
          </button>
        </div>
      </header>

      {error ? <StatusBanner tone="error" message={error} /> : null}
      {statusMessage ? <StatusBanner tone="success" message={statusMessage} /> : null}
      {pathIssueCount > 0 ? (
        <StatusBanner
          tone="warning"
          message={`${pathIssueCount} path ${pathIssueCount === 1 ? "issue" : "issues"} need attention before every source can scan cleanly.`}
        />
      ) : null}

      {isLoading ? (
        <section className="settings-card settings-loading-card">
          <p className="eyebrow">Loading</p>
          <h3>Reading settings</h3>
          <p>ModelHub is loading local settings and resolving effective paths.</p>
        </section>
      ) : draft ? (
        <div className="settings-layout">
          <section className="settings-card settings-card-large">
            <div className="section-heading-row">
              <div>
                <p className="eyebrow">Storage roots</p>
                <h3>Source paths</h3>
              </div>
              <span className="soft-pill">Non-secret config</span>
            </div>

            <div className="field-stack">
              <label className="field-label" htmlFor="hf-cache-path">
                Hugging Face cache path
              </label>
              <input
                className="text-input"
                id="hf-cache-path"
                onChange={(event) => updateDraft({ hfCachePath: event.target.value })}
                placeholder="Use HF_HUB_CACHE, HF_HOME, or default"
                value={draft.hfCachePath}
              />
              <p className="field-help">
                Leave blank to resolve from HF_HUB_CACHE, HF_HOME, then the
                default Windows cache path.
              </p>
            </div>

            <div className="field-stack">
              <label className="field-label" htmlFor="lm-studio-path">
                LM Studio models path
              </label>
              <input
                className="text-input"
                id="lm-studio-path"
                onChange={(event) =>
                  updateDraft({ lmStudioModelsPath: event.target.value })
                }
                placeholder="Use default .lmstudio models folder"
                value={draft.lmStudioModelsPath}
              />
              <p className="field-help">
                Leave blank to use %USERPROFILE%\.lmstudio\models.
              </p>
            </div>

            <div className="field-stack">
              <div className="section-heading-row compact-heading-row">
                <label className="field-label" htmlFor="custom-folder-input">
                  Custom model folders
                </label>
                <span className="soft-pill">{draft.customModelFolders.length}</span>
              </div>

              {draft.customModelFolders.length > 0 ? (
                <div className="custom-folder-list">
                  {draft.customModelFolders.map((folder, index) => (
                    <div className="custom-folder-row" key={`${folder}-${index}`}>
                      <input
                        aria-label={`Custom folder ${index + 1}`}
                        className="text-input"
                        onChange={(event) => updateCustomFolder(index, event.target.value)}
                        value={folder}
                      />
                      <button
                        className="danger-ghost-button"
                        onClick={() => removeCustomFolder(index)}
                        type="button"
                      >
                        Remove
                      </button>
                    </div>
                  ))}
                </div>
              ) : (
                <p className="field-help">No custom folders configured yet.</p>
              )}

              <div className="custom-folder-row add-folder-row">
                <input
                  className="text-input"
                  id="custom-folder-input"
                  onChange={(event) => setNewCustomFolder(event.target.value)}
                  placeholder="D:\\Models\\GGUF"
                  value={newCustomFolder}
                />
                <button
                  className="secondary-button"
                  disabled={!newCustomFolder.trim()}
                  onClick={addCustomFolder}
                  type="button"
                >
                  Add Folder
                </button>
              </div>
            </div>
          </section>

          <aside className="settings-side-stack">
            <section className="settings-card">
              <p className="eyebrow">Effective paths</p>
              <h3>Resolved scan roots</h3>
              <div className="resolved-path-list">
                {resolvedPaths ? (
                  <>
                    <ResolvedPathCard path={resolvedPaths.hfCache} />
                    <ResolvedPathCard path={resolvedPaths.lmStudioModels} />
                    {resolvedPaths.customModelFolders.length > 0 ? (
                      resolvedPaths.customModelFolders.map((path) => (
                        <ResolvedPathCard key={`${path.label}-${path.path ?? "none"}`} path={path} />
                      ))
                    ) : (
                      <div className="resolved-path-card quiet-card">
                        <span className="path-label">Custom folders</span>
                        <p>No custom scan roots configured.</p>
                      </div>
                    )}
                  </>
                ) : (
                  <p className="field-help">Resolved paths are unavailable.</p>
                )}
              </div>
            </section>

            <section className="settings-card">
              <p className="eyebrow">Privacy</p>
              <h3>Hugging Face token</h3>
              <p className="settings-copy">
                Token storage will use OS credentials in a later task. No token is
                written to the settings JSON.
              </p>
              <span className="token-status" data-active={settings?.hfTokenStored ?? false}>
                {settings?.hfTokenStored ? "Token stored" : "No token stored"}
              </span>
            </section>
          </aside>

          <section className="settings-card settings-card-full">
            <div className="section-heading-row">
              <div>
                <p className="eyebrow">Behavior</p>
                <h3>Desktop and filesystem behavior</h3>
              </div>
              <span className="soft-pill">MVP-safe defaults</span>
            </div>

            <div className="toggle-grid">
              <ToggleRow
                checked={draft.minimizeToTray}
                description="Closing the window hides it and keeps the tray companion running."
                label="Minimize to tray on close"
                onChange={(value) => updateDraft({ minimizeToTray: value })}
              />
              <ToggleRow
                checked={draft.scanOnStartup}
                description="Let the Local page refresh sources automatically when scanning lands."
                label="Scan on startup"
                onChange={(value) => updateDraft({ scanOnStartup: value })}
              />
              <ToggleRow
                checked={draft.enableSymlinkAttempt}
                description="Try HF snapshot symlinks first, then copy when Windows blocks symlink creation."
                label="Attempt HF cache symlinks"
                onChange={(value) => updateDraft({ enableSymlinkAttempt: value })}
              />
              <ToggleRow
                checked={draft.deleteUsesRecycleBin}
                description="Permanent delete is intentionally not exposed in the MVP."
                disabled
                label="Use Recycle Bin for deletes"
                onChange={(value) => updateDraft({ deleteUsesRecycleBin: value })}
              />
              <ToggleRow
                checked={false}
                description="Windows startup integration is post-MVP scope."
                disabled
                label="Start on login"
                onChange={() => undefined}
              />
              <ToggleRow
                checked={false}
                description="Telemetry is off and not implemented in this MVP."
                disabled
                label="Telemetry"
                onChange={() => undefined}
              />
            </div>
          </section>
        </div>
      ) : (
        <section className="settings-card settings-loading-card">
          <p className="eyebrow">Unavailable</p>
          <h3>Settings could not be loaded</h3>
          <p>Use Refresh after fixing the settings file or app config folder.</p>
        </section>
      )}
    </div>
  );
}

function StatusBanner({ tone, message }: { tone: "error" | "success" | "warning"; message: string }) {
  return (
    <section className="status-banner" data-tone={tone}>
      <strong>{toneLabel(tone)}</strong>
      <span>{message}</span>
    </section>
  );
}

function toneLabel(tone: "error" | "success" | "warning"): string {
  if (tone === "error") {
    return "Error";
  }

  if (tone === "success") {
    return "Saved";
  }

  return "Warning";
}

function ResolvedPathCard({ path }: { path: ResolvedPath }) {
  const issueTone = path.issues.some((issue) => issue.severity === "error")
    ? "error"
    : path.issues.length > 0
      ? "warning"
      : "ok";

  return (
    <article className="resolved-path-card" data-tone={issueTone}>
      <div className="section-heading-row compact-heading-row">
        <span className="path-label">{path.label}</span>
        <span className="source-chip">{path.sourceLabel}</span>
      </div>
      <code>{path.path ?? "Unresolved"}</code>
      <div className="path-meta-row">
        <span>{path.exists ? "Exists" : "Missing"}</span>
        <span>{path.isDirectory ? "Folder" : "Not confirmed"}</span>
      </div>
      {path.issues.length > 0 ? <IssueList issues={path.issues} /> : null}
    </article>
  );
}

function IssueList({ issues }: { issues: PathIssue[] }) {
  return (
    <ul className="issue-list">
      {issues.map((issue) => (
        <li data-severity={issue.severity} key={`${issue.code}-${issue.message}`}>
          {issue.message}
        </li>
      ))}
    </ul>
  );
}

function ToggleRow({
  checked,
  description,
  disabled = false,
  label,
  onChange,
}: {
  checked: boolean;
  description: string;
  disabled?: boolean;
  label: string;
  onChange: (checked: boolean) => void;
}) {
  return (
    <label className="toggle-row" data-disabled={disabled}>
      <span>
        <strong>{label}</strong>
        <small>{description}</small>
      </span>
      <input
        checked={checked}
        disabled={disabled}
        onChange={(event) => onChange(event.target.checked)}
        role="switch"
        type="checkbox"
      />
    </label>
  );
}

function countPathIssues(paths: ResolvedPaths): number {
  return [
    paths.hfCache,
    paths.lmStudioModels,
    ...paths.customModelFolders,
  ].reduce((count, path) => count + path.issues.length, 0);
}
