<#
.SYNOPSIS
    Replay a Native32 input movie (.nmov / .json) into a video.

.EXAMPLE
    powershell -File scripts/replay-movie.ps1 -Movie runs\clear.nmov
#>
param(
    [Parameter(Mandatory = $true)][string]$Movie,
    [string]$Game = "",
    [string]$OutputDir = "",
    [string]$Binary = "",
    [int]$DumpEvery = 2,
    [int]$MaxFrames = 120000,
    [switch]$SkipEncode
)

$ErrorActionPreference = "Stop"
$repoRoot = Split-Path -Parent $PSScriptRoot

if (-not (Test-Path -LiteralPath $Movie -PathType Leaf)) {
    Write-Error "Movie not found: $Movie"
    exit 1
}
if (-not $Binary) {
    $Binary = Join-Path $repoRoot "target\release\native32-emu.exe"
    if ($env:OS -ne "Windows_NT") {
        $Binary = Join-Path $repoRoot "target/release/native32-emu"
    }
}
if (-not (Test-Path -LiteralPath $Binary -PathType Leaf)) {
    Write-Error "native32-emu not found: $Binary (cargo build -p native32emu --release)"
    exit 1
}

if (-not $Game) {
    foreach ($line in Get-Content -LiteralPath $Movie -TotalCount 30) {
        if ($line -match '^\s*game:\s*(.+)$') {
            $Game = $Matches[1].Trim()
            break
        }
    }
}
if (-not $Game) {
    Write-Error "Pass -Game or add a 'game:' header to the movie"
    exit 1
}
if (-not (Test-Path -LiteralPath $Game -PathType Leaf)) {
    $alt = Join-Path $repoRoot $Game
    if (Test-Path -LiteralPath $alt -PathType Leaf) { $Game = $alt }
    else { Write-Error "Game not found: $Game"; exit 1 }
}

if (-not $OutputDir) {
    $stem = [System.IO.Path]::GetFileNameWithoutExtension($Movie)
    $OutputDir = Join-Path $repoRoot "tmp\input-replay\$stem"
}

$framesDir = Join-Path $OutputDir "frames"
$audioPath = Join-Path $OutputDir "audio.wav"
$videoPath = Join-Path $OutputDir "replay.mp4"
New-Item -ItemType Directory -Force -Path $framesDir | Out-Null
Get-ChildItem -LiteralPath $framesDir -Filter *.png -ErrorAction SilentlyContinue | Remove-Item -Force

Write-Host "Movie: $Movie"
Write-Host "Game:  $Game"
Write-Host "Out:   $OutputDir"

& $Binary --replay $Movie --dump-frames $framesDir --dump-every $DumpEvery `
    --record-audio $audioPath --max-frames $MaxFrames --volume 0 $Game
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

if ($SkipEncode) { exit 0 }

$ffmpeg = Get-Command ffmpeg -ErrorAction SilentlyContinue
if (-not $ffmpeg) {
    Write-Warning "ffmpeg not on PATH; frames in $framesDir"
    exit 0
}

$rate = [math]::Max(1, [int](30 / $DumpEvery))
$ffArgs = @("-y", "-framerate", "$rate", "-i", (Join-Path $framesDir "%06d.png"))
if (Test-Path -LiteralPath $audioPath) {
    $ffArgs += @("-i", $audioPath, "-c:a", "aac", "-b:a", "128k", "-shortest")
}
$ffArgs += @("-c:v", "libx264", "-pix_fmt", "yuv420p", "-crf", "18", "-movflags", "+faststart", $videoPath)
& $ffmpeg @ffArgs
if ($LASTEXITCODE -ne 0) { Write-Error "ffmpeg failed"; exit 1 }
Write-Host "Video: $videoPath"
