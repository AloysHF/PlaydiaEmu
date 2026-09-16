<#
.SYNOPSIS
    Batch-generate screenshots for Playdia games (zip / cue / iso / bin).

.DESCRIPTION
    Runs the playdia-emu binary in screenshot mode (--screenshot) for every
    .zip, .cue, .iso or .bin file found under the game directory. Output PNGs
    are saved to docs/images/, named after the game archive (without
    extension). When no binary is supplied, the latest release binary is built
    before capture.

    Redump-style ZIPs (CUE+BIN inside) are loaded directly — no extraction
    required. Do not commit disc images into the repository.

.PARAMETER Frames
    Number of frames to emulate before capturing. Default: 300 (10 seconds at
    30 fps — enough for slower title screens to appear). Known slower titles
    use tuned overrides when Frames is omitted.

.PARAMETER Binary
    Path to the playdia-emu binary. Default: cargo release build output.

.PARAMETER GameDir
    Directory containing game archives. Defaults to tmp\playdia_game when the
    private research path is not supplied via -GameDir / -RedumpDir.

.PARAMETER RedumpDir
    Optional path to a local Redump zip folder (private; not committed).
    When set, .zip files under this directory are used instead of GameDir.

.PARAMETER TimeoutSeconds
    Maximum time allowed for each game. Default: 300 seconds (large dual-track
    ZIPs take longer to open than single-file handheld images).

.PARAMETER SkipBuild
    Skip the cargo release rebuild and use the existing binary.

.EXAMPLE
    .\scripts\batch-screenshots.ps1 -RedumpDir "F:\Emulator\PlaydiaEmu\bandai-playdia-quick-redump"

.EXAMPLE
    .\scripts\batch-screenshots.ps1 -GameDir "tmp\playdia_game" -Frames 180
#>

param(
    [ValidateRange(1, [int]::MaxValue)]
    [int]$Frames = 300,

    [string]$Binary = "",

    [string]$GameDir = "",

    [string]$RedumpDir = "",

    [ValidateRange(1, [int]::MaxValue)]
    [int]$TimeoutSeconds = 300,

    [switch]$SkipBuild
)

$ErrorActionPreference = "Stop"
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)
$framesSpecified = $PSBoundParameters.ContainsKey("Frames")

$repoRoot = Split-Path -Parent $PSScriptRoot
$outDir = Join-Path $repoRoot "docs\images"

if (-not $RedumpDir) {
    if (-not $GameDir) {
        $GameDir = Join-Path $repoRoot "tmp\playdia_game"
    }
    if (-not (Test-Path -LiteralPath $GameDir -PathType Container)) {
        Write-Error "Game directory not found: $GameDir`nPass -RedumpDir or -GameDir pointing at a folder of .zip/.cue discs."
        exit 1
    }
    $scanRoot = (Resolve-Path -LiteralPath $GameDir).Path
    $gameFilter = "*.zip", "*.cue", "*.iso", "*.bin"
} else {
    if (-not (Test-Path -LiteralPath $RedumpDir -PathType Container)) {
        Write-Error "Redump directory not found: $RedumpDir"
        exit 1
    }
    $scanRoot = (Resolve-Path -LiteralPath $RedumpDir).Path
    $gameFilter = "*.zip"
}

New-Item -ItemType Directory -Force -Path $outDir | Out-Null

if (-not $Binary) {
    $Binary = Join-Path $repoRoot "target\release\playdia-emu.exe"
    if (-not $SkipBuild) {
        Write-Host "Building the latest release binary..." -ForegroundColor Yellow
        try {
            Push-Location $repoRoot
            cargo build --release -p playdiaemu
            if ($LASTEXITCODE -ne 0) {
                throw "Release build failed with exit code $LASTEXITCODE."
            }
        } finally {
            Pop-Location
        }
    }
}

if (-not (Test-Path -LiteralPath $Binary -PathType Leaf)) {
    Write-Error "Emulator binary not found: $Binary"
    exit 1
}

$Binary = (Resolve-Path -LiteralPath $Binary).Path

function ConvertTo-ScreenshotName {
    param([string]$BaseName)

    # Keep names Markdown-safe: no spaces or path/link punctuation.
    $safeName = $BaseName -replace '\s+', '_'
    $safeName = $safeName -replace '[<>:"/\\|?*()\[\]{}!#''&%\x00-\x1F]', ''
    $safeName = $safeName -replace '_+', '_'
    $safeName = $safeName.Trim('_').TrimEnd([char[]]@('.', ' '))

    if ([string]::IsNullOrWhiteSpace($safeName)) {
        return "game"
    }

    return $safeName
}

function Get-CaptureFrames {
    param(
        [string]$Name,
        [int]$DefaultFrames,
        [bool]$UseTitleScreenOverrides
    )

    if (-not $UseTitleScreenOverrides) {
        return $DefaultFrames
    }

    switch -Wildcard ($Name) {
        "*Aqua Adventure*" { return 360 }
        "*Sample Soft*" { return 180 }
        "*Mari-nee*" { return 180 }
        "*Dragon Ball Z*Chikyuu-hen*" { return 600 }
        "*Dragon Ball Z*" { return 300 }
        "*Chougoukin*" { return 400 }
        "*Gamera*" { return 400 }
        "*Hello Kitty*" { return 400 }
        "*Nintama Rantarou*" { return 400 }
        default { return $DefaultFrames }
    }
}

function Get-PressSequence {
    param(
        [string]$Name,
        [bool]$UseTitleScreenOverrides
    )

    # Titles that sit on the BANDAI boot logo until a button advances them.
    if (-not $UseTitleScreenOverrides) {
        return @()
    }

    switch -Wildcard ($Name) {
        "*Chougoukin*" { return @("120:a", "240:a") }
        "*Gamera*" { return @("120:a", "240:a") }
        "*Hello Kitty*" { return @("120:a", "240:a") }
        "*Nintama Rantarou*" { return @("120:a", "240:a") }
        default { return @() }
    }
}

function Invoke-ScreenshotCapture {
    param(
        [string]$Executable,
        [string]$GamePath,
        [string]$ScreenshotPath,
        [int]$CaptureFrames,
        [int]$Timeout,
        [string[]]$Presses = @()
    )

    $argString = '"{0}" --screenshot "{1}" --screenshot-frames {2}' -f $GamePath, $ScreenshotPath, $CaptureFrames
    foreach ($press in $Presses) {
        $argString += ' --press-at {0}' -f $press
    }

    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = $Executable
    # Quote paths so Windows PowerShell 5.1 handles spaces without ArgumentList.
    $psi.Arguments = $argString
    $psi.UseShellExecute = $false
    $psi.CreateNoWindow = $true
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    $psi.EnvironmentVariables["RUST_LOG"] = "warn"
    $psi.EnvironmentVariables["RUST_LOG_STYLE"] = "never"

    $process = New-Object System.Diagnostics.Process
    $process.StartInfo = $psi

    try {
        if (-not $process.Start()) {
            throw "Failed to start emulator process."
        }

        $stdoutTask = $process.StandardOutput.ReadToEndAsync()
        $stderrTask = $process.StandardError.ReadToEndAsync()

        if (-not $process.WaitForExit($Timeout * 1000)) {
            try { $process.Kill() } catch { }
            $process.WaitForExit()
            return [pscustomobject]@{
                ExitCode = $null
                TimedOut = $true
                Output = "Timed out after $Timeout seconds."
            }
        }

        $stdout = $stdoutTask.GetAwaiter().GetResult()
        $stderr = $stderrTask.GetAwaiter().GetResult()
        $output = (@($stdout.Trim(), $stderr.Trim()) | Where-Object { $_ }) -join [Environment]::NewLine

        return [pscustomobject]@{
            ExitCode = $process.ExitCode
            TimedOut = $false
            Output = $output
        }
    } finally {
        $process.Dispose()
    }
}

Write-Host "Using binary: $Binary"
Write-Host "Scan root:    $scanRoot"
Write-Host "Output dir:   $outDir"
if ($framesSpecified) {
    Write-Host "Frames:       $Frames"
} else {
    Write-Host "Frames:       $Frames (with title-screen overrides)"
}
Write-Host "Timeout:      $TimeoutSeconds seconds per game"
Write-Host ""

$games = @(
    foreach ($filter in $gameFilter) {
        Get-ChildItem -LiteralPath $scanRoot -Filter $filter -File |
            Where-Object {
                # Skip nested track bins when scanning an extracted tree
                $_.Extension -ne ".bin" -or $_.Name -notmatch '\(Track\s+\d+\)'
            }
    }
) | Sort-Object Name

if ($games.Count -eq 0) {
    Write-Warning "No game archives found under $scanRoot"
    exit 0
}

Write-Host "Found $($games.Count) game(s).`n"

$success = 0
$failed = 0
$usedNames = [System.Collections.Generic.HashSet[string]]::new(
    [System.StringComparer]::OrdinalIgnoreCase
)

foreach ($game in $games) {
    $baseName = [System.IO.Path]::GetFileNameWithoutExtension($game.Name)
    $safeName = ConvertTo-ScreenshotName $baseName
    $captureFrames = Get-CaptureFrames $game.Name $Frames (-not $framesSpecified)
    $presses = Get-PressSequence $game.Name (-not $framesSpecified)

    $uniqueName = $safeName
    $suffix = 2
    while (-not $usedNames.Add($uniqueName)) {
        $uniqueName = "${safeName}_$suffix"
        $suffix++
    }
    $safeName = $uniqueName
    $outPath = Join-Path $outDir "$safeName.png"

    if (Test-Path -LiteralPath $outPath) {
        Remove-Item -LiteralPath $outPath -Force
    }

    $pressLabel = ""
    if ($presses.Count -gt 0) {
        $pressLabel = " + " + ($presses -join " ")
    }
    Write-Host -NoNewline "  $baseName ($captureFrames frames$pressLabel) ... "

    try {
        $result = Invoke-ScreenshotCapture `
            -Executable $Binary `
            -GamePath $game.FullName `
            -ScreenshotPath $outPath `
            -CaptureFrames $captureFrames `
            -Timeout $TimeoutSeconds `
            -Presses $presses

        if ($result.TimedOut) {
            Write-Host "FAILED (timeout)" -ForegroundColor Red
            $failed++
        } elseif ($result.ExitCode -ne 0) {
            Write-Host "FAILED (exit $($result.ExitCode))" -ForegroundColor Red
            if ($result.Output) {
                $detailLines = @($result.Output -split '\r?\n' |
                    Where-Object { $_ } |
                    Select-Object -Last 3)
                foreach ($line in $detailLines) {
                    Write-Host "    $line" -ForegroundColor DarkGray
                }
            }
            $failed++
        } elseif ((Test-Path -LiteralPath $outPath -PathType Leaf) -and
            (Get-Item -LiteralPath $outPath).Length -gt 0) {
            $size = (Get-Item -LiteralPath $outPath).Length
            Write-Host "OK ($([math]::Round($size / 1KB)) KB)" -ForegroundColor Green
            $success++
        } else {
            Write-Host "FAILED (missing or empty screenshot)" -ForegroundColor Red
            $failed++
        }
    } catch {
        Write-Host "FAILED ($($_.Exception.Message))" -ForegroundColor Red
        $failed++
    }
}

Write-Host ""
Write-Host "Done: $success succeeded, $failed failed out of $($games.Count) total."

if ($failed -gt 0) {
    exit 1
}
