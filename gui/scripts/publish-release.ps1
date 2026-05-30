#Requires -Version 5.1
<#
.SYNOPSIS
  self-contained 配布物（ZIP）をビルドする。

.DESCRIPTION
  Release / win-x64 / self-contained で publish し、release/ の同梱ファイルと
  まとめて dist/ に ZIP を出力する。既定は単一 exe（PublishSingleFile）。

.PARAMETER Format
  Single（既定）または Folder。

.PARAMETER Version
  パッケージ名に使うバージョン。省略時は gui/Directory.Build.props の Version を使用。

.PARAMETER SkipZip
  ZIP 作成を省略する。

.EXAMPLE
  .\gui\scripts\publish-release.ps1

.EXAMPLE
  .\gui\scripts\publish-release.ps1 -Format Folder
#>
[CmdletBinding()]
param(
    [ValidateSet('Single', 'Folder')]
    [string] $Format = 'Single',

    [string] $Version = '',

    [switch] $SkipZip
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$GuiDir = (Resolve-Path (Join-Path $ScriptDir '..')).Path
$RepoRoot = (Resolve-Path (Join-Path $GuiDir '..')).Path
$AppProject = Join-Path $GuiDir 'NowPlayingVrcOsc.App\NowPlayingVrcOsc.App.csproj'
$ReleaseDir = Join-Path $RepoRoot 'release'
$DistRoot = Join-Path $RepoRoot 'dist'
$ExeName = 'NowPlayingVrcOsc.exe'
$Runtime = 'win-x64'
$IconScript = Join-Path $ScriptDir 'generate-app-icon.ps1'
$BuildProps = Join-Path $GuiDir 'Directory.Build.props'

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

$PackageName = ('nowplaying-vrc-osc-gui-v{0}-{1}' -f $Version, $Runtime)
$PackageDir = Join-Path $DistRoot $PackageName
$ZipPath = Join-Path $DistRoot ('{0}.zip' -f $PackageName)
$PublishDir = Join-Path $GuiDir ('NowPlayingVrcOsc.App\bin\Release\net10.0-windows\{0}\publish' -f $Runtime)

Write-Host ''
Write-Host '=== Now Playing -> VRChat: release publish ==='
Write-Host "Version : $Version"
Write-Host "Format  : $Format"
Write-Host "Output  : $PackageDir"
Write-Host ''

Write-Host '=== [1/6] app.ico を生成 ==='
$iconPng = Join-Path $RepoRoot 'resources\icon1024.png'
if (-not (Test-Path -LiteralPath $iconPng)) {
    $iconPng = Join-Path $RepoRoot 'docs\icon1024.png'
}
if (-not (Test-Path -LiteralPath $iconPng)) {
    throw "icon1024.png が見つかりません: resources/ または docs/"
}
& $IconScript -InputPath $iconPng

Write-Host ''
Write-Host '=== [2/6] 実行中のアプリを終了 ==='
Get-Process -Name 'NowPlayingVrcOsc', 'NowPlayingVrcOsc.App' -ErrorAction SilentlyContinue |
    Stop-Process -Force -ErrorAction SilentlyContinue

Write-Host ''
Write-Host '=== [3/6] dotnet publish ==='
$publishArgs = @(
    'publish', $AppProject,
    '-c', 'Release',
    '-r', $Runtime,
    '--self-contained', 'true',
    '-p:DebugType=none',
    '-p:DebugSymbols=false',
    '-p:PublishReadyToRun=true'
)

if ($Format -eq 'Single') {
    $publishArgs += @(
        '-p:PublishSingleFile=true',
        '-p:IncludeNativeLibrariesForSelfExtract=true',
        '-p:EnableCompressionInSingleFile=true'
    )
}

Push-Location $GuiDir
try {
    & dotnet @publishArgs
    if ($LASTEXITCODE -ne 0) {
        throw "dotnet publish が失敗しました (exit $LASTEXITCODE)。"
    }
}
finally {
    Pop-Location
}

if (-not (Test-Path -LiteralPath $PublishDir)) {
    throw "publish 出力が見つかりません: $PublishDir"
}

Write-Host ''
Write-Host '=== [4/6] 配布フォルダを組み立て ==='
if (Test-Path -LiteralPath $PackageDir) {
    Remove-Item -LiteralPath $PackageDir -Recurse -Force
}
New-Item -ItemType Directory -Path $PackageDir -Force | Out-Null

$publishedExe = Join-Path $PublishDir $ExeName
if (-not (Test-Path -LiteralPath $publishedExe)) {
    throw "publish 出力の exe が見つかりません: $publishedExe"
}

if ($Format -eq 'Single') {
    # 単一 exe: dotnet publish が空の Assets/ 等を残すことがあるため exe のみ同梱
    Copy-Item -LiteralPath $publishedExe -Destination $PackageDir -Force
}
else {
    Copy-Item -Path (Join-Path $PublishDir '*') -Destination $PackageDir -Recurse -Force
}

$readmeTemplate = Join-Path $ReleaseDir 'README.ja.txt'
$noticesTemplate = Join-Path $ReleaseDir 'THIRD-PARTY-NOTICES.txt'
$licenceTemplate = Join-Path $ReleaseDir 'LICENCE.txt'
foreach ($template in @($readmeTemplate, $noticesTemplate, $licenceTemplate)) {
    if (-not (Test-Path -LiteralPath $template)) {
        throw "同梱テンプレートが見つかりません: $template"
    }
}

$readmeText = (Get-Content -LiteralPath $readmeTemplate -Raw -Encoding UTF8) `
    -replace '\{\{VERSION\}\}', $Version `
    -replace '\{\{FORMAT\}\}', $Format
$readmeText | Set-Content -LiteralPath (Join-Path $PackageDir 'README.ja.txt') -Encoding UTF8 -NoNewline

Copy-Item -LiteralPath $noticesTemplate -Destination (Join-Path $PackageDir 'THIRD-PARTY-NOTICES.txt') -Force
Copy-Item -LiteralPath $licenceTemplate -Destination (Join-Path $PackageDir 'LICENCE.txt') -Force

Write-Host ''
Write-Host '=== [5/6] スモークテスト ==='
$exePath = Join-Path $PackageDir $ExeName
if (-not (Test-Path -LiteralPath $exePath)) {
    throw "配布 exe が見つかりません: $exePath"
}

$exeInfo = Get-Item -LiteralPath $exePath
if ($exeInfo.Length -lt 1MB) {
    throw "exe のサイズが小さすぎます ($($exeInfo.Length) bytes)。publish を確認してください。"
}

$fileCount = @(Get-ChildItem -LiteralPath $PackageDir -File -Recurse).Count
$exeSizeText = Format-Bytes $exeInfo.Length
Write-Host ('OK: {0} ({1})' -f $ExeName, $exeSizeText)
Write-Host "Package files: $fileCount"

if ($Format -eq 'Single') {
    $dllCount = @(Get-ChildItem -LiteralPath $PackageDir -Filter '*.dll' -File).Count
    if ($dllCount -gt 0) {
        Write-Warning "単一 exe 形式ですが DLL が $($dllCount) 個あります。必要に応じて -Format Folder を検討してください。"
    }
}

Write-Host ''
Write-Host '=== [6/6] ZIP 作成 ==='
if ($SkipZip) {
    Write-Host 'SkipZip が指定されたため ZIP は作成しません。'
}
else {
    if (Test-Path -LiteralPath $ZipPath) {
        Remove-Item -LiteralPath $ZipPath -Force
    }
    Compress-Archive -Path $PackageDir -DestinationPath $ZipPath -CompressionLevel Optimal
    $zipInfo = Get-Item -LiteralPath $ZipPath
    Write-Host ('ZIP: {0} ({1})' -f $ZipPath, (Format-Bytes $zipInfo.Length))
}

Write-Host ''
Write-Host '=== 完了 ==='
Write-Host "Folder: $PackageDir"
if (-not $SkipZip) {
    Write-Host "ZIP   : $ZipPath"
}
Write-Host ''
Write-Host '次の確認:'
Write-Host '  1. .NET 未インストールの PC で ZIP を解凍して起動'
Write-Host '  2. NP2 + VRChat で曲切替・テスト送信'
Write-Host '  3. 単一 exe で問題があれば: .\gui\scripts\publish-release.ps1 -Format Folder'
Write-Host ''
