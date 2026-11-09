#Requires -Version 5.1
<#
.SYNOPSIS
  Velopack のインストーラー（Setup.exe）と更新パッケージを dist/velopack に作成する。

.DESCRIPTION
  Release / win-x64 / self-contained（フォルダ形式）で publish し、vpk pack で
  Setup.exe・Portable.zip・nupkg・releases.win.json を生成する。
  -Upload 指定時は GitHub Releases へ公開する（既定では何もアップロードしない）。
  vpk 1.2.161 が必要: dotnet tool install -g vpk --version 1.2.161

.PARAMETER Version
  パッケージのバージョン。省略時は gui/Directory.Build.props の Version を使用。

.PARAMETER SkipDownload
  差分パッケージ用の既存リリース取得（vpk download github）を省略する。

.PARAMETER Upload
  生成物を GitHub Releases にアップロードして公開する。
  トークンは環境変数 GITHUB_TOKEN、なければ gh auth token を使用。

.EXAMPLE
  .\gui\scripts\publish-installer.ps1 -SkipDownload

.EXAMPLE
  .\gui\scripts\publish-installer.ps1 -Upload
#>
[CmdletBinding()]
param(
    [string] $Version = '',

    [switch] $SkipDownload,

    [switch] $Upload
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$GuiDir = (Resolve-Path (Join-Path $ScriptDir '..')).Path
$RepoRoot = (Resolve-Path (Join-Path $GuiDir '..')).Path
$AppProject = Join-Path $GuiDir 'NowPlayingVrcOsc.App\NowPlayingVrcOsc.App.csproj'
$ReleaseDir = Join-Path $RepoRoot 'release'
$DistRoot = Join-Path $RepoRoot 'dist'
$PublishDir = Join-Path $DistRoot 'velopack-publish'
$OutDir = Join-Path $DistRoot 'velopack'
$IconPath = Join-Path $GuiDir 'NowPlayingVrcOsc.App\Assets\app.ico'
$IconScript = Join-Path $ScriptDir 'generate-app-icon.ps1'
$BuildProps = Join-Path $GuiDir 'Directory.Build.props'
$Runtime = 'win-x64'
$PackId = 'NowPlayingVrcOsc'
$PackTitle = 'DJ Now Playing for VRChat'
$PackAuthors = 'tokage works'
$RepoUrl = 'https://github.com/khlizard/nowplaying-vrc-osc'
$VpkVersion = '1.2.161'

function Get-ProjectVersion {
    param([string] $PropsPath)
    if (-not (Test-Path -LiteralPath $PropsPath)) {
        throw "Directory.Build.props が見つかりません: $PropsPath"
    }
    $content = Get-Content -LiteralPath $PropsPath -Raw
    if ($content -match '<Version>([^<]+)</Version>') {
        return $Matches[1].Trim()
    }
    throw "Directory.Build.props から Version を読み取れません。"
}

function Format-Bytes {
    param([long] $Size)
    if ($Size -ge 1MB) {
        return '{0:N1} MB' -f ($Size / 1MB)
    }
    if ($Size -ge 1KB) {
        return '{0:N1} KB' -f ($Size / 1KB)
    }
    return ('{0} B' -f $Size)
}

if ([string]::IsNullOrWhiteSpace($Version)) {
    $Version = Get-ProjectVersion -PropsPath $BuildProps
}

Write-Host ''
Write-Host '=== Now Playing -> VRChat: Velopack installer ==='
Write-Host "Version : $Version"
Write-Host "Output  : $OutDir"
Write-Host ''

Write-Host '=== [1/8] vpk を確認 ==='
$fixCommand = "dotnet tool install -g vpk --version $VpkVersion  (導入済みなら: dotnet tool update -g vpk --version $VpkVersion)"
if (-not (Get-Command vpk -ErrorAction SilentlyContinue)) {
    throw "vpk が見つかりません。次を実行してください: $fixCommand"
}
# vpk には --version が無く、ヘルプの先頭行に "Velopack CLI x.y.z" が出る
$vpkHelp = (& vpk -h 2>&1 | Out-String)
if ($vpkHelp -notmatch 'Velopack CLI ([0-9][0-9A-Za-z\.\-\+]*)') {
    throw "vpk のバージョンを取得できません。次を実行してください: $fixCommand"
}
$vpkActual = $Matches[1]
if ($vpkActual -ne $VpkVersion) {
    throw "vpk のバージョンが一致しません (必要: $VpkVersion / 実際: $vpkActual)。次を実行してください: $fixCommand"
}
Write-Host "OK: vpk $vpkActual"

Write-Host ''
Write-Host '=== [2/8] app.ico を生成 ==='
$iconPng = Join-Path $RepoRoot 'resources\icon1024.png'
if (-not (Test-Path -LiteralPath $iconPng)) {
    throw "icon1024.png が見つかりません: resources/"
}
& $IconScript -InputPath $iconPng
if (-not (Test-Path -LiteralPath $IconPath)) {
    throw "app.ico が見つかりません: $IconPath"
}

Write-Host ''
Write-Host '=== [3/8] 実行中のアプリを終了 ==='
Get-Process -Name 'NowPlayingVrcOsc', 'NowPlayingVrcOsc.App' -ErrorAction SilentlyContinue |
    Stop-Process -Force -ErrorAction SilentlyContinue

Write-Host ''
Write-Host '=== [4/8] dotnet publish ==='
# Dropbox / ウイルス対策による一時的なロックに備えてリトライする
for ($i = 1; Test-Path -LiteralPath $PublishDir; $i++) {
    try {
        Remove-Item -LiteralPath $PublishDir -Recurse -Force
    }
    catch {
        if ($i -ge 5) { throw }
        Start-Sleep -Seconds 2
    }
}
New-Item -ItemType Directory -Path $PublishDir -Force | Out-Null

Push-Location $GuiDir
try {
    & dotnet publish $AppProject `
        -c Release -r $Runtime --self-contained true `
        -p:PublishReadyToRun=true -p:PublishSingleFile=false `
        -p:DebugType=none -p:DebugSymbols=false `
        -o $PublishDir
    if ($LASTEXITCODE -ne 0) {
        throw "dotnet publish が失敗しました (exit $LASTEXITCODE)。"
    }
}
finally {
    Pop-Location
}

foreach ($name in @('LICENCE.txt', 'THIRD-PARTY-NOTICES.txt')) {
    $src = Join-Path $ReleaseDir $name
    if (-not (Test-Path -LiteralPath $src)) {
        throw "同梱ファイルが見つかりません: $src"
    }
    Copy-Item -LiteralPath $src -Destination $PublishDir -Force
}

if (-not (Test-Path -LiteralPath (Join-Path $PublishDir 'NowPlayingVrcOsc.exe'))) {
    throw "publish 出力に NowPlayingVrcOsc.exe がありません: $PublishDir"
}

Write-Host ''
Write-Host '=== [5/8] 既存リリースを取得（差分パッケージ用） ==='
# 同一バージョンの前回出力が残っていると vpk pack が失敗するため、毎回空にする
if (Test-Path -LiteralPath $OutDir) {
    Remove-Item -LiteralPath $OutDir -Recurse -Force
}
New-Item -ItemType Directory -Path $OutDir -Force | Out-Null
if ($SkipDownload) {
    Write-Host 'SkipDownload が指定されたため取得しません。'
}
else {
    & vpk download github --repoUrl $RepoUrl --pre -o $OutDir
    if ($LASTEXITCODE -ne 0) {
        Write-Warning "既存リリースの取得に失敗しました (exit $LASTEXITCODE)。差分なし（フルパッケージのみ）で続行します。"
    }
}

Write-Host ''
Write-Host '=== [6/8] vpk pack ==='
$packArgs = @(
    'pack',
    '-u', $PackId,
    '-v', $Version,
    '-p', $PublishDir,
    '-e', 'NowPlayingVrcOsc.exe',
    '--runtime', $Runtime,
    '--packTitle', $PackTitle,
    '--packAuthors', $PackAuthors,
    '-i', $IconPath,
    '--shortcuts', 'Desktop,StartMenuRoot',
    '-o', $OutDir
)
$notesPath = Join-Path $ReleaseDir ('notes\{0}.md' -f $Version)
if (Test-Path -LiteralPath $notesPath) {
    Write-Host "リリースノート: $notesPath"
    $packArgs += @('--releaseNotes', $notesPath)
}
& vpk @packArgs
if ($LASTEXITCODE -ne 0) {
    throw "vpk pack が失敗しました (exit $LASTEXITCODE)。"
}

Write-Host ''
Write-Host '=== [7/8] GitHub Releases へアップロード ==='
if (-not $Upload) {
    Write-Host 'Upload が指定されていないためアップロードしません。'
}
else {
    $token = $env:GITHUB_TOKEN
    if ([string]::IsNullOrWhiteSpace($token) -and (Get-Command gh -ErrorAction SilentlyContinue)) {
        $token = (& gh auth token 2>$null | Out-String).Trim()
    }
    if ([string]::IsNullOrWhiteSpace($token)) {
        throw "GitHub トークンがありません。GITHUB_TOKEN を設定するか gh auth login を実行してください。"
    }

    $uploadArgs = @(
        'upload', 'github',
        '-o', $OutDir,
        '--repoUrl', $RepoUrl,
        '--token', $token,
        '--publish',
        '--releaseName', "$PackTitle v$Version",
        '--tag', "gui-v$Version"
    )
    if ($Version.Contains('-')) {
        $uploadArgs += '--pre'
    }
    & vpk @uploadArgs
    if ($LASTEXITCODE -ne 0) {
        throw "vpk upload が失敗しました (exit $LASTEXITCODE)。"
    }
}

Write-Host ''
Write-Host '=== [8/8] 出力ファイル ==='
Get-ChildItem -LiteralPath $OutDir -File |
    Where-Object { $_.Name -match 'Setup\.exe$|Portable\.zip$|\.nupkg$|^releases\..+\.json$|^RELEASES' } |
    ForEach-Object { Write-Host ('{0}  ({1})' -f $_.FullName, (Format-Bytes $_.Length)) }

Write-Host ''
Write-Host '=== 完了 ==='
Write-Host ''
