import { relaunch } from "@tauri-apps/plugin-process";
import { check, type CheckOptions, type DownloadEvent, type Update } from "@tauri-apps/plugin-updater";
import { safeStorage } from "./safeStorage";

const AUTO_CHECK_KEY = "update.autoCheckEnabled";
const IGNORED_VERSION_KEY = "update.ignoredVersion";
const LAST_CHECK_KEY = "update.lastCheckAt";
const AUTO_CHECK_INTERVAL_MS = 24 * 60 * 60 * 1000;

export interface UpdatePreferences {
  autoCheckEnabled: boolean;
  ignoredVersion: string | null;
  lastCheckAt: string | null;
}

export interface UpdateSummary {
  currentVersion: string;
  version: string;
  date?: string;
  body?: string;
}

export function loadUpdatePreferences(): UpdatePreferences {
  return {
    autoCheckEnabled: safeStorage.getItem(AUTO_CHECK_KEY) !== "0",
    ignoredVersion: safeStorage.getItem(IGNORED_VERSION_KEY),
    lastCheckAt: safeStorage.getItem(LAST_CHECK_KEY),
  };
}

export function saveUpdatePreferences(preferences: UpdatePreferences): void {
  safeStorage.setItem(AUTO_CHECK_KEY, preferences.autoCheckEnabled ? "1" : "0");
  if (preferences.ignoredVersion) {
    safeStorage.setItem(IGNORED_VERSION_KEY, preferences.ignoredVersion);
  } else {
    safeStorage.removeItem(IGNORED_VERSION_KEY);
  }
  if (preferences.lastCheckAt) {
    safeStorage.setItem(LAST_CHECK_KEY, preferences.lastCheckAt);
  } else {
    safeStorage.removeItem(LAST_CHECK_KEY);
  }
}

export function shouldAutoCheckForUpdates(preferences: UpdatePreferences, now = Date.now()): boolean {
  if (!preferences.autoCheckEnabled) {
    return false;
  }
  if (!preferences.lastCheckAt) {
    return true;
  }
  const lastCheckTime = Date.parse(preferences.lastCheckAt);
  if (Number.isNaN(lastCheckTime)) {
    return true;
  }
  return now - lastCheckTime >= AUTO_CHECK_INTERVAL_MS;
}

export function summarizeUpdate(update: Update): UpdateSummary {
  return {
    currentVersion: update.currentVersion,
    version: update.version,
    date: update.date,
    body: update.body,
  };
}

export async function checkForUpdates(options?: CheckOptions): Promise<Update | null> {
  return check({
    timeout: 15000,
    ...options,
  });
}

export async function installUpdate(
  update: Update,
  onEvent?: (event: DownloadEvent) => void
): Promise<void> {
  await update.downloadAndInstall(onEvent);
  await relaunch();
}

export function formatVersionLabel(version: string): string {
  return version.startsWith("v") ? version : `v${version}`;
}
