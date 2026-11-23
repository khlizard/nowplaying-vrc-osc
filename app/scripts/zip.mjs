// release ビルドして、BOOTH で配る ZIP（exe と README・ライセンス）を dist\ に作る。
// 作業は Dropbox の外（%LOCALAPPDATA%\<package.json の name>）で行い、できた ZIP だけをコピーする。
//
// 使い方（app\ で）:
//   npm run zip
//   npm run zip -- --skip-build   # ビルド済みの exe を使う
import { spawnSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, readFileSync, rmSync, statSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { join, resolve } from "node:path";

const EXE = "NowPlayingVrcOsc.exe";

const skipBuild = process.argv.includes("--skip-build");
const pkg = JSON.parse(readFileSync("package.json", "utf8"));
const version = pkg.version;
const repoRoot = resolve("..");
const releaseDir = join(repoRoot, "release");
const base = join(process.env.LOCALAPPDATA ?? join(homedir(), ".cache"), pkg.name);
const targetDir = process.env.CARGO_TARGET_DIR ?? join(base, "target");
const packageName = `nowplaying-vrc-osc-gui-v${version}-win-x64`;
const work = join(base, "zip");
const packageDir = join(work, packageName);
const zipName = `${packageName}.zip`;

function fail(message) {
  console.error(message);
  process.exit(1);
}

if (!skipBuild) {
  // npm は .cmd なのでシェル経由で起動する。
  const result = spawnSync("npm run tauri build -- --no-bundle", { stdio: "inherit", shell: true });
  if (result.status !== 0) fail("ビルドに失敗しました。");
}

const exe = join(targetDir, "release", EXE);
if (!existsSync(exe)) fail(`exe が見つかりません: ${exe}`);

rmSync(work, { recursive: true, force: true });
mkdirSync(packageDir, { recursive: true });
copyFileSync(exe, join(packageDir, EXE));

const readme = readFileSync(join(releaseDir, "README.ja.txt"), "utf8").replaceAll("{{VERSION}}", version);
writeFileSync(join(packageDir, "README.ja.txt"), readme, "utf8");
for (const name of ["LICENCE.txt", "THIRD-PARTY-NOTICES.txt"]) {
  copyFileSync(join(releaseDir, name), join(packageDir, name));
}

// Windows 10 以降の tar（bsdtar）で ZIP にする。-a で拡張子から形式を決める。フォルダごと入れる。
// Git Bash の GNU tar は ZIP を作れないので、System32 のものを使う。
const tarExe = join(process.env.SystemRoot ?? "C:\\Windows", "System32", "tar.exe");
const tar = spawnSync(tarExe,["-a", "-c", "-f", zipName, packageName], { cwd: work, stdio: "inherit" });
if (tar.status !== 0) fail("ZIP を作れませんでした。");

const dist = join(repoRoot, "dist");
mkdirSync(dist, { recursive: true });
copyFileSync(join(work, zipName), join(dist, zipName));
const size = (statSync(join(dist, zipName)).size / 1024 / 1024).toFixed(1);
console.log(`できました: ${join(dist, zipName)} (${size} MB)`);
