<#
.SYNOPSIS
    Batch-capture screenshots for every SMF game file.

.DESCRIPTION
    Runs the standalone emulator for every SMF file under the selected game
    directory. PNG screenshots are written to docs/images, preserving the
    subdirectory structure. When no binary is supplied, the latest release
    binary is built before capture.

    Capture settings:
      --auto-skip-cutscenes : skip intro/transition (.mpg) videos so the shot
                              lands on actual game content, not a logo/cutscene.
      --screenshot-frames N : run N emulated frames (30 fps) before capturing.
                              200 frames (~6.7s) lets games settle on their
                              title/menu screen after the cutscene is skipped.
    Output resolution is the native 320x240 (scale 1).

.PARAMETER Frames
    Number of frames to run before capturing. Default: 200. Selected
    applications use capture-specific frame counts.

.PARAMETER Binary
    Path to the native32-emu executable. Default: the release build output.

.PARAMETER GameDirectory
    Directory searched recursively for SMF files.

.PARAMETER OutputDirectory
    Directory where PNG screenshots are written (preserves subdirectory
    structure relative to GameDirectory).

.EXAMPLE
    # Default (release binary, 200 frames)
    powershell -ExecutionPolicy Bypass -File scripts/batch-screenshots.ps1

    # Use the debug binary with a custom frame count
    powershell -ExecutionPolicy Bypass -File scripts/batch-screenshots.ps1 -Binary target\debug\native32-emu.exe -Frames 300

.NOTES
    Runs ~84 games sequentially; the batch takes roughly 10 minutes because
    capture mode runs in real time (30 fps).
#>

param(
    [ValidateRange(0, [int]::MaxValue)]
    [int]$Frames = 200,
    [string]$Binary = "",
    [string]$GameDirectory = "",
    [string]$OutputDirectory = ""
)

$ErrorActionPreference = "Stop"
$repoRoot = Split-Path -Parent $PSScriptRoot

# Resolve default paths
if (-not $GameDirectory) {
    $GameDirectory = Join-Path $repoRoot "tmp\native32_game"
}
if (-not $OutputDirectory) {
    $OutputDirectory = Join-Path $repoRoot "docs\images"
}
if (-not (Test-Path -LiteralPath $GameDirectory -PathType Container)) {
    Write-Error "Game directory not found: $GameDirectory"
    exit 1
}

New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null

# Build the release binary if no explicit binary was provided
if (-not $Binary) {
    $Binary = Join-Path $repoRoot "target\release\native32-emu.exe"
    if ($env:OS -ne "Windows_NT") {
        $Binary = Join-Path $repoRoot "target/release/native32-emu"
    }

    Write-Host "Building the latest release binary..." -ForegroundColor Yellow
    try {
        Push-Location $repoRoot
        cargo build --release -p native32emu --bin native32-emu
        if ($LASTEXITCODE -ne 0) {
            throw "Release build failed with exit code $LASTEXITCODE."
        }
    } finally {
        Pop-Location
    }
}

if (-not (Test-Path -LiteralPath $Binary -PathType Leaf)) {
    Write-Error "native32-emu binary not found: $Binary"
    exit 1
}

# Discover SMF files recursively
$games = @(Get-ChildItem -LiteralPath $GameDirectory -Recurse -File |
    Where-Object { $_.Extension -ieq ".smf" } |
    Sort-Object FullName)

if ($games.Count -eq 0) {
    Write-Warning "No SMF files found under $GameDirectory"
    exit 0
}

# Sanitize a filename component so it is safe on all platforms
function Get-SafeBaseName {
    param(
        [Parameter(Mandatory)]
        [string]$BaseName
    )

    $safeName = $BaseName -replace '[<>:"/\\|?*\x00-\x1F]', '_'
    $safeName = $safeName.Trim().TrimEnd('.')
    if (-not $safeName) {
        return "unnamed"
    }
    return $safeName
}

Write-Host "Using binary: $Binary"
Write-Host "Game dir:     $GameDirectory"
Write-Host "Output dir:   $OutputDirectory"
Write-Host "Frames:       $Frames"
Write-Host "Games:        $($games.Count)"
Write-Host ""

$success = 0
$failed = 0

foreach ($game in $games) {
    # Compute the relative path so output mirrors the game directory structure
    $relativePath = $game.FullName.Substring($GameDirectory.Length).TrimStart('\', '/')
    $relativeDir = Split-Path $relativePath -Parent

    $baseName = [System.IO.Path]::GetFileNameWithoutExtension($game.Name)
    $safeName = Get-SafeBaseName $baseName

    # Per-game frame overrides for titles that need more or fewer frames
    $captureFrames = switch ($baseName) {
        "EBBLADE"   { 300; break }
        "EGUNFIRE"  { 300; break }
        "EMETAL"    { 300; break }
        "ESTORM"    { 300; break }
        default     { $Frames }
    }
    $frameLabel = if ($captureFrames -eq 1) { "1 frame" } else { "$captureFrames frames" }

    # Build the output path, preserving subdirectory structure
    $outDir = if ($relativeDir) {
        Join-Path $OutputDirectory $relativeDir
    } else {
        $OutputDirectory
    }
    New-Item -ItemType Directory -Force -Path $outDir | Out-Null

    $imageName = "$safeName.png"
    $outPath = Join-Path $outDir $imageName
    Write-Host -NoNewline "  $relativePath ($frameLabel) ... "

    # Remove stale screenshot so a previous capture is never reported as current
    if (Test-Path -LiteralPath $outPath -PathType Leaf) {
        Remove-Item -LiteralPath $outPath
    }

    $previousErrorActionPreference = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try {
        & $Binary --auto-skip-cutscenes --screenshot $outPath `
            --screenshot-frames $captureFrames $game.FullName *> $null

        if ($LASTEXITCODE -ne 0 -or -not (Test-Path -LiteralPath $outPath -PathType Leaf)) {
            $failed++
            Write-Host "FAIL (exit $LASTEXITCODE)" -ForegroundColor Red
        } else {
            $success++
            $size = (Get-Item -LiteralPath $outPath).Length
            Write-Host "OK ($([math]::Round($size / 1KB)) KB)" -ForegroundColor Green
        }
    } catch {
        $failed++
        Write-Host "FAILED ($($_.Exception.Message))" -ForegroundColor Red
    } finally {
        $ErrorActionPreference = $previousErrorActionPreference
    }
}

Write-Host ""
Write-Host "Done: $success succeeded, $failed failed out of $($games.Count) total."
