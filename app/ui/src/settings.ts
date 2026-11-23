import {
  browseDbPath,
  closeSettings,
  errorText,
  getKeySaved,
  getSettings,
  previewMessage,
  resolveDbPath,
  saveSettings,
  showWarning,
  type AppSettings,
} from "./api";

const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;
const input = (id: string) => $<HTMLInputElement>(id);

const dbPath = input("db-path");
const poll = input("poll");
const key = input("key");
const host = input("host");
const port = input("port");
const notification = input("notification");
const indicator = input("indicator");
const header = input("header");
const separator = input("separator");
const incTitle = input("inc-title");
const incArtist = input("inc-artist");
const incAlbum = input("inc-album");
const incLabel = input("inc-label");
const incComment = input("inc-comment");
const strip = input("strip");
const dbHint = $("db-hint");
const keyStatus = $("key-status");
const preview = $("preview");
const okBtn = $<HTMLButtonElement>("ok");

// C# の double.TryParse(Float, InvariantCulture) 相当（前後空白可、指数可、カンマ不可）
function parseFloatLike(s: string): number | null {
  if (!/^\s*[+-]?(\d+\.?\d*|\.\d+)([eE][+-]?\d+)?\s*$/.test(s)) return null;
  const n = Number(s.trim());
  // 無限大は settings.json に書けない（JSON に無い）ので不正とする。
  return Number.isFinite(n) ? n : null;
}
// C# の int.TryParse(Integer, InvariantCulture) 相当
function parseIntLike(s: string): number | null {
  if (!/^\s*[+-]?\d+\s*$/.test(s)) return null;
  const n = Number(s.trim());
  return n >= -2147483648 && n <= 2147483647 ? n : null;
}

function loadFields(s: AppSettings) {
  dbPath.value = s.DbPathOverride;
  poll.value = String(s.PollIntervalSeconds);
  host.value = s.VrcHost;
  port.value = String(s.VrcPort);
  notification.checked = s.ChatboxNotification;
  indicator.checked = s.ChatboxIndicator;
  header.value = s.MsgHeader;
  separator.value = s.MsgSeparator;
  incTitle.checked = s.MsgIncludeTitle;
  incArtist.checked = s.MsgIncludeArtist;
  incAlbum.checked = s.MsgIncludeAlbum;
  incLabel.checked = s.MsgIncludeLabel;
  incComment.checked = s.MsgIncludeComment;
  strip.checked = s.StripTitleBrackets;
}

/** BuildSettingsFromFields と同じ規則でフォーム値から設定を作る。 */
function buildSettings(): AppSettings {
  const p = parseFloatLike(poll.value);
  const pt = parseIntLike(port.value);
  return {
    DbPathOverride: dbPath.value.trim(),
    PollIntervalSeconds: p !== null && p > 0 ? p : 0.5,
    VrcHost: host.value.trim(),
    VrcPort: pt !== null && pt >= 1 && pt <= 65535 ? pt : 9000,
    ChatboxNotification: notification.checked,
    ChatboxIndicator: indicator.checked,
    MsgHeader: header.value,
    MsgSeparator: separator.value.trim(),
    MsgIncludeTitle: incTitle.checked,
    MsgIncludeArtist: incArtist.checked,
    MsgIncludeAlbum: incAlbum.checked,
    MsgIncludeLabel: incLabel.checked,
    MsgIncludeComment: incComment.checked,
    StripTitleBrackets: strip.checked,
    ShowLabel: false,
  };
}

// 古い応答で新しい表示を上書きしないよう連番で管理する
let previewSeq = 0;
async function updatePreview() {
  const seq = ++previewSeq;
  let text: string;
  try {
    text = await previewMessage(buildSettings());
  } catch {
    text = "（プレビュー不可）";
  }
  if (seq === previewSeq) preview.textContent = text;
}

let hintSeq = 0;
async function updateHint() {
  const seq = ++hintSeq;
  let auto: string;
  try {
    auto = await resolveDbPath(dbPath.value.trim());
  } catch (e) {
    auto = errorText(e);
  }
  if (seq === hintSeq) dbHint.textContent = `空欄なら自動検出: ${auto}`;
}

/** 警告を出してから該当欄にフォーカスする。 */
async function invalid(message: string, target: HTMLElement) {
  await showWarning(message).catch(() => {});
  target.focus();
}

async function onOk() {
  if (okBtn.disabled) return;
  okBtn.disabled = true;
  try {
    const p = parseFloatLike(poll.value);
    if (p === null || !(p > 0)) return await invalid("ポーリング間隔には正の数値を入力してください。", poll);
    const pt = parseIntLike(port.value);
    if (pt === null || pt < 1 || pt > 65535)
      return await invalid("VRChat ポートには 1〜65535 の値を入力してください。", port);
    if (host.value.trim() === "") return await invalid("VRChat ホストを入力してください。", host);
    if (separator.value.trim() === "") return await invalid("区切り文字を入力してください。", separator);
    if (![incTitle, incArtist, incAlbum, incLabel, incComment].some((c) => c.checked))
      return await invalid("表示する項目を 1 つ以上選んでください。", incTitle);

    const result = await saveSettings(buildSettings(), key.value.trim());
    if (result.saved) return; // Rust 側が設定ウィンドウを閉じる
    if (result.error !== null) {
      await invalid(result.error, result.focus === "key" ? key : poll);
    }
  } catch (e) {
    await showWarning(errorText(e)).catch(() => {});
  } finally {
    okBtn.disabled = false;
  }
}

// フォームの submit（Enter / OK ボタン）で OK 処理
$("form").addEventListener("submit", (e) => {
  e.preventDefault();
  void onOk();
});
$("cancel").addEventListener("click", () => void closeSettings());
document.addEventListener("keydown", (e) => {
  if (e.key === "Escape") {
    e.preventDefault();
    void closeSettings();
  }
});

$("browse").addEventListener("click", async () => {
  try {
    const picked = await browseDbPath(dbPath.value);
    if (picked !== null) {
      dbPath.value = picked;
      void updateHint();
    }
  } catch (e) {
    await showWarning(errorText(e)).catch(() => {});
  }
});

dbPath.addEventListener("input", () => void updateHint());
for (const el of [header, separator, incTitle, incArtist, incAlbum, incLabel, incComment, strip]) {
  el.addEventListener("input", () => void updatePreview());
  el.addEventListener("change", () => void updatePreview());
}

async function init() {
  loadFields(await getSettings());
  void updateHint();
  void updatePreview();
  const saved = await getKeySaved().catch(() => false);
  keyStatus.textContent = saved ? "保存済みのキーがあります" : "未保存（通常は自動取得されます）";
}
init().catch((e) => void showWarning(errorText(e)).catch(() => {}));
