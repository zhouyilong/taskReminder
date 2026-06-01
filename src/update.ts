import { relaunch } from "@tauri-apps/plugin-process";
import {
  check,
  type CheckOptions,
  type DownloadEvent,
  type DownloadOptions,
  type Update
} from "@tauri-apps/plugin-updater";
import { safeStorage } from "./safeStorage";

const AUTO_CHECK_KEY = "update.autoCheckEnabled";
const IGNORED_VERSION_KEY = "update.ignoredVersion";
const LAST_CHECK_KEY = "update.lastCheckAt";
const PROXY_URL_KEY = "update.proxyUrl";
const AUTO_CHECK_INTERVAL_MS = 24 * 60 * 60 * 1000;

export interface UpdatePreferences {
  autoCheckEnabled: boolean;
  ignoredVersion: string | null;
  lastCheckAt: string | null;
  proxyUrl: string | null;
}

export interface UpdateSummary {
  currentVersion: string;
  version: string;
  date?: string;
  body?: string;
}

type UpdateNetworkOptions = Pick<CheckOptions, "proxy"> & Pick<DownloadOptions, "proxy">;

export function normalizeUpdateProxyUrl(proxyUrl: string | null | undefined): string | null {
  const normalized = proxyUrl?.trim();
  return normalized ? normalized : null;
}

export function resolveUpdateNetworkOptions(
  proxyUrl: string | null | undefined
): UpdateNetworkOptions | undefined {
  const normalizedProxyUrl = normalizeUpdateProxyUrl(proxyUrl);
  if (!normalizedProxyUrl) {
    return undefined;
  }
  return { proxy: normalizedProxyUrl };
}

export function loadUpdatePreferences(): UpdatePreferences {
  return {
    autoCheckEnabled: safeStorage.getItem(AUTO_CHECK_KEY) !== "0",
    ignoredVersion: safeStorage.getItem(IGNORED_VERSION_KEY),
    lastCheckAt: safeStorage.getItem(LAST_CHECK_KEY),
    proxyUrl: normalizeUpdateProxyUrl(safeStorage.getItem(PROXY_URL_KEY)),
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
  const normalizedProxyUrl = normalizeUpdateProxyUrl(preferences.proxyUrl);
  if (normalizedProxyUrl) {
    safeStorage.setItem(PROXY_URL_KEY, normalizedProxyUrl);
  } else {
    safeStorage.removeItem(PROXY_URL_KEY);
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
  onEvent?: (event: DownloadEvent) => void,
  options?: DownloadOptions
): Promise<void> {
  await update.downloadAndInstall(onEvent, options);
  await relaunch();
}

export function formatVersionLabel(version: string): string {
  return version.startsWith("v") ? version : `v${version}`;
}
