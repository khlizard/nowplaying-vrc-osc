#Requires -Version 5.1
<#
.SYNOPSIS
  docs/icon1024.png から Windows アプリ用の multi-size .ico を生成する。

.DESCRIPTION
  ImageMagick (magick) を使い、トレイ・タスクバー・exe アイコン向けの
  標準サイズ (16〜256) を 1 つの .ico にまとめる。

  既定の入出力:
    入力:  <repo>/docs/icon1024.png
    出力:  <repo>/gui/NowPlayingVrcOsc.App/Assets/app.ico

.EXAMPLE
  .\gui\scripts\generate-app-icon.ps1

.EXAMPLE
  .\gui\scripts\generate-app-icon.ps1 -InputPath docs\icon1024.png -OutputPath gui\NowPlayingVrcOsc.App\Assets\app.ico
#>
[CmdletBinding()]
param(
    [Parameter()]
    [string] $InputPath,

    [Parameter()]
    [string] $OutputPath
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Get-RepoRoot {
    $dir = Split-Path -Parent $PSScriptRoot  # gui/
    return Split-Path -Parent $dir            # repo root
}

function Assert-Magick {
    $magick = Get-Command magick -ErrorAction SilentlyContinue
    if (-not $magick) {
        throw @"
ImageMagick (magick) が見つかりません。
https://imagemagick.org/ からインストールするか、PATH に magick を追加してください。
"@
    }
}

$repoRoot = Get-RepoRoot

if (-not $InputPath) {
    $InputPath = Join-Path $repoRoot 'docs\icon1024.png'
}
elseif (-not [System.IO.Path]::IsPathRooted($InputPath)) {
    $InputPath = Join-Path $repoRoot $InputPath
}

if (-not $OutputPath) {
    $OutputPath = Join-Path $repoRoot 'gui\NowPlayingVrcOsc.App\Assets\app.ico'
}
elseif (-not [System.IO.Path]::IsPathRooted($OutputPath)) {
    $OutputPath = Join-Path $repoRoot $OutputPath
}

if (-not (Test-Path -LiteralPath $InputPath)) {
    throw "入力 PNG が見つかりません: $InputPath"
}

Assert-Magick

$outputDir = Split-Path -Parent $OutputPath
if (-not (Test-Path -LiteralPath $outputDir)) {
    New-Item -ItemType Directory -Path $outputDir -Force | Out-Null
}

# Windows 向けの一般的な ICO サイズ (大→小)
$iconSizes = '256,128,96,64,48,32,24,16'

Write-Host "入力:  $InputPath"
Write-Host "出力:  $OutputPath"
Write-Host "サイズ: $iconSizes"

$resizeDefine = "icon:auto-resize=$iconSizes"

& magick $InputPath -define $resizeDefine $OutputPath

if (-not (Test-Path -LiteralPath $OutputPath)) {
    throw "ICO の生成に失敗しました: $OutputPath"
}

$info = & magick identify $OutputPath 2>&1
Write-Host ''
Write-Host '生成完了:'
Write-Host $info
