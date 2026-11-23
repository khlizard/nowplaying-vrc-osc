// release ビルドして、Velopack のインストーラー（Setup.exe）と更新パッケージを作る。
// 作業は Dropbox の外（%LOCALAPPDATA%\<package.json の name>）で行い、できたものをリポジトリの dist\velopack にコピーする。
// Dropbox 内で作ると、できたばかりのファイルを同期が掴んで失敗するため。
//
// 使い方（app\ で）:
//   npm run pack                       # 既存リリースを取得して差分パッケージも作る
//   npm run pack -- --skip-download    # 既存リリースを取得しない
//   npm run pack -- --upload           # GitHub Releases に公開し、タグ gui-v<version> を作る
// vpk 1.2.161 が必要: dotnet tool install -g vpk --version 1.2.161
import { spawnSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { join, resolve } from "node:path";

// 変えると、インストール済みの版が自動更新されなくなる。
const PACK_ID = "NowPlayingVrcOsc";
const EXE = `${PACK_ID}.exe`;
const PACK_TITLE = "DJ Now Playing for VRChat";
const PACK_AUTHORS = "tokage works";
const REPO_URL = "https://github.com/khlizard/nowplaying-vrc-osc";
const VPK_VERSION = "1.2.161";

const args = new Set(process.argv.slice(2));
const skipDownload = args.has("--skip-download");
const upload = args.has("--upload");

const pkg = JSON.parse(readFileSync("package.json", "utf8"));
const version = pkg.version;
const repoRoot = resolve("..");
const releaseDir = join(repoRoot, "release");
const base = join(process.env.LOCALAPPDATA ?? join(homedir(), ".cache"), pkg.name);
const targetDir = process.env.CARGO_TARGET_DIR ?? join(base, "target");
const staging = join(base, "pack");
const outDir = join(base, "velopack");
const distDir = join(repoRoot, "dist", "velopack");

function fail(message) {
  console.error(message);
  process.exit(1);
}

// vpk はトークンを渡すので、シェルを通さずに起動する。
function run(command, commandArgs) {
  const result = spawnSync(command, commandArgs, { stdio: "inherit" });
  return result.status ?? 1;
}

function step(title) {
  console.log(`\n=== ${title} ===`);
}

/** CHANGELOG.md の `## <version>` の節の本文。無いか空なら null。 */
function changelogSection(text, ver) {
  const lines = text.split(/\r?\n/);
  const start = lines.findIndex((line) => line.trim() === `## ${ver}`);
  if (start < 0) return null;
  const rest = lines.slice(start + 1);
  const end = rest.findIndex((line) => line.startsWith("## "));
  const body = (end < 0 ? rest : rest.slice(0, end)).join("\n").trim();
  return body === "" ? null : `${body}\n`;
}

// リリースノート（更新の説明）は CHANGELOG.md から取る。書き忘れはビルドの前に止める。
const notesBody = changelogSection(readFileSync(join(repoRoot, "CHANGELOG.md"), "utf8"), version);
if (notesBody === null) fail(`CHANGELOG.md に「## ${version}」の節がありません。`);

step("vpk を確認");
const help = spawnSync("vpk", ["-h"], { encoding: "utf8" });
const found = /Velopack CLI ([0-9][0-9A-Za-z.\-+]*)/.exec(`${help.stdout ?? ""}${help.stderr ?? ""}`);
const fix = `dotnet tool install -g vpk --version ${VPK_VERSION}（導入済みなら dotnet tool update -g vpk --version ${VPK_VERSION}）`;
if (!found) fail(`vpk が見つかりません。次を実行してください: ${fix}`);
if (!found[1].startsWith(VPK_VERSION)) {
  fail(`vpk ${found[1]} が入っています。Rust の velopack クレートと合わせて ${VPK_VERSION} にしてください: ${fix}`);
}
console.log(`vpk ${found[1]}`);

step("release ビルド");
// npm は .cmd なのでシェル経由で起動する（引数を配列で渡すと Node が非推奨の警告を出すので 1 本の文字列にする）。
if (spawnSync("npm run tauri build -- --no-bundle", { stdio: "inherit", shell: true }).status !== 0) {
  fail("ビルドに失敗しました。");
}

step("同梱するファイルを置く");
// 前回の残りが混ざらないよう、毎回作り直す。
rmSync(staging, { recursive: true, force: true });
mkdirSync(staging, { recursive: true });
copyFileSync(join(targetDir, "release", EXE), join(staging, EXE));
for (const name of ["LICENCE.txt", "THIRD-PARTY-NOTICES.txt"]) {
  const src = join(releaseDir, name);
  if (!existsSync(src)) fail(`同梱ファイルが見つかりません: ${src}`);
  copyFileSync(src, join(staging, name));
}

step("既存リリースを取得（差分パッケージ用）");
// 同じバージョンの前回出力が残っていると vpk pack が失敗するので、毎回空にする。
rmSync(outDir, { recursive: true, force: true });
mkdirSync(outDir, { recursive: true });
if (skipDownload) {
  console.log("--skip-download が指定されたため取得しません。");
} else if (run("vpk", ["download", "github", "--repoUrl", REPO_URL, "--pre", "-o", outDir]) !== 0) {
  console.warn("既存リリースを取得できませんでした。差分なし（フルパッケージのみ）で続けます。");
}

step("vpk pack");
const packArgs = [
  "pack",
  "-u", PACK_ID,
  "-v", version,
  "-p", staging,
  "-e", EXE,
  "--runtime", "win-x64",
  "--packTitle", PACK_TITLE,
  "--packAuthors", PACK_AUTHORS,
  "-i", resolve("src-tauri/icons/icon.ico"),
  "--shortcuts", "Desktop,StartMenuRoot",
  // 画面は WebView2 で描く。Windows 10 などで入っていなければ Setup.exe が入れる。
  "--framework", "webview2",
  "-o", outDir,
];
const notes = join(base, "release-notes.md");
writeFileSync(notes, notesBody, "utf8");
packArgs.push("--releaseNotes", notes);
if (run("vpk", packArgs) !== 0) fail("vpk pack に失敗しました。");

step("GitHub Releases へアップロード");
if (!upload) {
  console.log("--upload が指定されていないためアップロードしません。");
} else {
  let token = process.env.GITHUB_TOKEN?.trim();
  if (!token) {
    const gh = spawnSync("gh", ["auth", "token"], { encoding: "utf8" });
    token = gh.status === 0 ? gh.stdout.trim() : "";
  }
  if (!token) fail("GitHub トークンがありません。GITHUB_TOKEN を設定するか gh auth login を実行してください。");
  const uploadArgs = [
    "upload", "github",
    "-o", outDir,
    "--repoUrl", REPO_URL,
    "--token", token,
    "--publish",
    "--releaseName", `${PACK_TITLE} v${version}`,
    "--tag", `gui-v${version}`,
  ];
  if (version.includes("-")) uploadArgs.push("--pre");
  if (run("vpk", uploadArgs) !== 0) fail("vpk upload に失敗しました。");
}

step("出力ファイル");
rmSync(distDir, { recursive: true, force: true });
mkdirSync(distDir, { recursive: true });
const wanted = /Setup\.exe$|Portable\.zip$|\.nupkg$|^releases\..+\.json$|^RELEASES/;
for (const name of readdirSync(outDir).filter((n) => wanted.test(n))) {
  copyFileSync(join(outDir, name), join(distDir, name));
  console.log(join(distDir, name));
}
