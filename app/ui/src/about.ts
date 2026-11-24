import {
  checkForUpdates,
  errorText,
  getAboutState,
  onAboutState,
  type AboutState,
  type UpdateStatus,
} from "./api";

const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;

const productEl = $("product");
const versionEl = $("version");
const statusEl = $("update-status");
const checkBtn = $<HTMLButtonElement>("check");

/** 更新の状態を、表示する文と［更新を確認］を押せるかに変える。 */
function describe(update: UpdateStatus): { text: string; canCheck: boolean } {
  switch (update.kind) {
    case "notInstalled":
      return { text: "インストーラー版でないため、ここでは確認できません。", canCheck: false };
    case "idle":
      return { text: "", canCheck: true };
    case "checking":
      return { text: "確認しています…", canCheck: false };
    case "upToDate":
      return { text: "最新のバージョンです。", canCheck: true };
    case "downloaded":
      return {
        text: `新しいバージョン ${update.version} を取得しました。アプリを終了すると更新されます。`,
        canCheck: false,
      };
    case "failed":
      return { text: `確認できませんでした: ${update.message}`, canCheck: true };
  }
}

/** 状態全体を描き直す。 */
function render(s: AboutState) {
  productEl.textContent = s.productName;
  versionEl.textContent = s.version;
  const { text, canCheck } = describe(s.update);
  statusEl.textContent = text;
  checkBtn.disabled = !canCheck;
}

checkBtn.addEventListener("click", () => {
  checkForUpdates().catch((e) => (statusEl.textContent = errorText(e)));
});

async function init() {
  await onAboutState(render);
  render(await getAboutState());
}
init().catch((e) => (statusEl.textContent = errorText(e)));
