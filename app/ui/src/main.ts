import {
  applyUpdateAndRestart,
  errorText,
  getMainState,
  onMainState,
  showMainMenu,
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
const menuBtn = $<HTMLButtonElement>("menu");

const orDash = (s: string) => (s === "" ? "—" : s);

/** 状態全体を描き直す。 */
function render(s: MainState) {
  statusEl.textContent = orDash(s.status);
  trackEl.textContent = orDash(s.track);
  lastSentEl.textContent = orDash(s.lastSent);
  startBtn.disabled = s.running;
  stopBtn.disabled = !s.running;
  if (s.updateVersion !== null) {
    updateText.textContent = `v${s.updateVersion} を取得しました。終了時に更新します。`;
  }
  updatePanel.hidden = s.updateVersion === null;
  // 更新パネルが出ている間は、曲名と最終送信を 1 行に詰める。
  document.body.classList.toggle("has-update", s.updateVersion !== null);
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

// メニューはボタンの左下に開く。
menuBtn.addEventListener("click", () => {
  const r = menuBtn.getBoundingClientRect();
  showMainMenu(r.left, r.bottom).catch((e) => (statusEl.textContent = errorText(e)));
});

updateButton.addEventListener("click", () => {
  applyUpdateAndRestart().catch((e) => (statusEl.textContent = errorText(e)));
});

async function init() {
  await onMainState(render);
  render(await getMainState());
}
init().catch((e) => (statusEl.textContent = errorText(e)));
