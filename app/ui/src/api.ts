import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type { UnlistenFn };

export type MainState = {
  running: boolean;
  status: string;
  track: string;
  lastSent: string;
  updateVersion: string | null;
};

/** 更新の確認の状態（Rust の UpdateStatus）。 */
export type UpdateStatus =
  | { kind: "notInstalled" }
  | { kind: "idle" }
  | { kind: "checking" }
  | { kind: "upToDate" }
  | { kind: "downloaded"; version: string }
  | { kind: "failed"; message: string };

export type AboutState = {
  productName: string;
  version: string;
  update: UpdateStatus;
};

// Rust の AppSettings をそのまま渡す（キーは PascalCase。v2 の settings.json と同じ形）
export type AppSettings = {
  DbPathOverride: string;
  PollIntervalSeconds: number;
  VrcHost: string;
  VrcPort: number;
  ChatboxNotification: boolean;
  ChatboxIndicator: boolean;
  MsgHeader: string;
  MsgSeparator: string;
  MsgIncludeTitle: boolean;
  MsgIncludeArtist: boolean;
  MsgIncludeAlbum: boolean;
  MsgIncludeLabel: boolean;
  MsgIncludeComment: boolean;
  StripTitleBrackets: boolean;
  ShowLabel: boolean;
};

export type SaveResult = { saved: boolean; error: string | null; focus: "key" | null };

/** エラーを表示用の文字列にする。 */
export const errorText = (error: unknown): string =>
  error instanceof Error ? error.message : String(error);

// --- メイン画面 ---
export const getMainState = () => invoke<MainState>("get_main_state");
export const startBridge = () => invoke<void>("start_bridge");
export const stopBridge = () => invoke<void>("stop_bridge");
export const testSend = () => invoke<string | null>("test_send");
export const showMainMenu = (x: number, y: number) => invoke<void>("show_main_menu", { x, y });
export const applyUpdateAndRestart = () => invoke<void>("apply_update_and_restart");
export const onMainState = (cb: (state: MainState) => void): Promise<UnlistenFn> =>
  listen<MainState>("main-state", (e) => cb(e.payload));

// --- このソフトについて ---
export const getAboutState = () => invoke<AboutState>("get_about_state");
export const checkForUpdates = () => invoke<void>("check_for_updates");
export const onAboutState = (cb: (state: AboutState) => void): Promise<UnlistenFn> =>
  listen<AboutState>("about-state", (e) => cb(e.payload));

// --- 設定画面 ---
export const getSettings = () => invoke<AppSettings>("get_settings");
export const getKeySaved = () => invoke<boolean>("get_key_saved");
export const resolveDbPath = (overridePath: string) =>
  invoke<string>("resolve_db_path", { overridePath });
export const previewMessage = (settings: AppSettings) =>
  invoke<string>("preview_message", { settings });
export const browseDbPath = (current: string) =>
  invoke<string | null>("browse_db_path", { current });
export const saveSettings = (settings: AppSettings, key: string) =>
  invoke<SaveResult>("save_settings", { settings, key });
export const showWarning = (message: string) => invoke<void>("show_warning", { message });
export const closeSettings = () => invoke<void>("close_settings");
