import {
  applyUpdateAndRestart,
  errorText,
  getMainState,
  onMainState,
  openSettings,
  startBridge,
  stopBridge,
  testSend,
  type MainState,
} from "./api";

const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;

const statusEl = $("status");
const trackEl = $("track");
const lastSentEl = $("last-sent");
const updatePanel = $("update-panel");
const updateText = $("update-text");
const updateButton = $<HTMLButtonElement>("update-button");
const startBtn = $<HTMLButtonElement>("start");
const stopBtn = $<HTMLButtonElement>("stop");
const testBtn = $<HTMLButtonElement>("test-send");
const settingsBtn = $<HTMLButtonElement>("settings");
const versionEl = $("version");

const orDash = (s: string) => (s === "" ? "—" : s);

/** 状態全体を描き直す。 */
function render(s: MainState) {
  statusEl.textContent = orDash(s.status);
  trackEl.textContent = orDash(s.track);
  lastSentEl.textContent = orDash(s.lastSent);
  startBtn.disabled = s.running;
  stopBtn.disabled = !s.running;
  versionEl.textContent = s.version;
  if (s.updateVersion !== null) {
    updateText.textContent = `新しいバージョン ${s.updateVersion} を取得しました。アプリを終了すると更新されます。`;
    updatePanel.hidden = false;
  } else {
    updatePanel.hidden = true;
  }
}

startBtn.addEventListener("click", async () => {
  statusEl.textContent = "接続中…";
  startBtn.disabled = true;
  try {
    await startBridge();
  } catch (e) {
    statusEl.textContent = errorText(e);
    startBtn.disabled = false;
  }
});

stopBtn.addEventListener("click", async () => {
  try {
    await stopBridge();
  } catch (e) {
    statusEl.textContent = errorText(e);
  }
});

testBtn.addEventListener("click", async () => {
  testBtn.disabled = true;
  try {
    const sent = await testSend();
    if (sent !== null) lastSentEl.textContent = orDash(sent);
  } catch (e) {
    statusEl.textContent = errorText(e);
  } finally {
    testBtn.disabled = false;
  }
});

settingsBtn.addEventListener("click", () => {
  openSettings().catch((e) => (statusEl.textContent = errorText(e)));
});

updateButton.addEventListener("click", () => {
  applyUpdateAndRestart().catch((e) => (statusEl.textContent = errorText(e)));
});

async function init() {
  await onMainState(render);
  render(await getMainState());
}
init().catch((e) => (statusEl.textContent = errorText(e)));
