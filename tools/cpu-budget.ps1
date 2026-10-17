# SPDX-FileCopyrightText: Copyright (c) 2026
# SPDX-License-Identifier: GPL-3.0-only
<#
.SYNOPSIS
    Fail the build if the window burns CPU or opens console windows.

.DESCRIPTION
    Exists because the screenshot harness could not see either problem. It
    launches the window hidden and photographs one settled frame, so a program
    that spawns a visible console per `adb` call, or that spins a core at 87%,
    produces a perfect picture of a broken application. Both defects shipped in
    v0.1.0.

    So this measures what a picture cannot: how much CPU a *single thread* of the
    running process consumes over a wall-clock window, and whether any new console
    host appeared while it ran.

    Three assertions, each a real defect this project has had:

      * **CPU per thread.** A GUI program with no work to do is a few percent of
        one core. A thread that holds ~90% for thirty seconds is a spin, and the
        measurement is per *thread* on purpose: the process total hides which one,
        and a repaint storm and a busy loop then look the same.
      * **No new `conhost.exe`.** Every console allocated for a child process
        brings one. This is how the missing `CREATE_NO_WINDOW` was found: the
        home page spawns three or four `adb` per five-second tick, and every one
        of them took the foreground.
      * **The window answers.** `Responding` is a coarse test, but a window that
        cannot answer a message within five seconds is frozen whatever the CPU
        says.

.EXAMPLE
    powershell -NoProfile -File tools\cpu-budget.ps1
    powershell -NoProfile -File tools\cpu-budget.ps1 -Seconds 60 -MaxPercentOfOneCore 10
#>
[CmdletBinding()]
param(
    [string] $Exe = 'target\release\wsadashboard.exe',
    [int] $Seconds = 30,
    [double] $MaxPercentOfOneCore = 20,
    [int] $MaxNewConsoles = 0,
    [int] $SampleEverySeconds = 5,
    # A window that hides to the tray keeps running; the poll and the timer are
    # still live, which is the state a reader leaves it in and the one this gate
    # should also cover.
    [switch] $SkipClose
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

if (-not (Test-Path $Exe)) { throw "missing $Exe; run build.ps1 first" }
$exePath = (Resolve-Path $Exe).Path
$failures = @()

function Sample {
    param($Proc)
    $Proc.Refresh()
    $total = $Proc.TotalProcessorTime.TotalMilliseconds
    $threads = @{}
    foreach ($t in $Proc.Threads) { $threads[$t.Id] = $t.TotalProcessorTime.TotalMilliseconds }
    [pscustomobject]@{ Total = $total; Threads = $threads }
}

Write-Verbose "closing any instance left from a previous run"
Get-Process wsadashboard -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Milliseconds 400

$conhostBefore = @(Get-Process conhost -ErrorAction SilentlyContinue).Count
$proc = Start-Process -FilePath $exePath -PassThru
$started = Get-Date

# Let the first paint and the first poll land before the baseline, so the check
# measures the steady state rather than startup.
Start-Sleep -Seconds ([Math]::Min(3, $Seconds))
$first = Sample $proc
$firstAt = Get-Date

$rows = @()
$peakThread = 0.0
$peakThreadId = 0
$peakTotal = 0.0
$maxConsoles = 0

while (((Get-Date) - $firstAt).TotalSeconds -lt $Seconds) {
    Start-Sleep -Seconds $SampleEverySeconds
    if ($proc.HasExited) { break }

    $now = Sample $proc
    $elapsed = ((Get-Date) - $firstAt).TotalMilliseconds
    $totalPct = 100.0 * ($now.Total - $first.Total) / $elapsed

    $hotId = 0; $hotMs = 0
    foreach ($id in $now.Threads.Keys) {
        $before = if ($first.Threads.ContainsKey($id)) { $first.Threads[$id] } else { 0 }
        $delta = $now.Threads[$id] - $before
        if ($delta -gt $hotMs) { $hotMs = $delta; $hotId = $id }
    }
    $hotPct = 100.0 * $hotMs / $elapsed

    $consoles = @(Get-Process conhost -ErrorAction SilentlyContinue).Count - $conhostBefore
    if ($consoles -gt $maxConsoles) { $maxConsoles = $consoles }
    if ($hotPct -gt $peakThread) { $peakThread = $hotPct; $peakThreadId = $hotId }
    if ($totalPct -gt $peakTotal) { $peakTotal = $totalPct }

    $rows += [pscustomobject]@{
        Seconds    = [int]((Get-Date) - $firstAt).TotalSeconds
        ThreadPct  = [Math]::Round($hotPct, 1)
        HottestId  = $hotId
        ProcessPct = [Math]::Round($totalPct, 1)
        Conhost    = $consoles
    }
}

if (-not $SkipClose -and -not $proc.HasExited) {
    # Hiding to the tray is what the close button does; the poll keeps running
    # there, so the CPU budget applies there too.
    $proc.Refresh()
    try { $null = $proc.CloseMainWindow() } catch { }
    Start-Sleep -Seconds 2
}

$proc.Refresh()
$responding = if ($proc.HasExited) { 'exited' } else { $proc.Responding }
$exitCode = if ($proc.HasExited) { $proc.ExitCode } else { $null }
if (-not $proc.HasExited) { $proc | Stop-Process -Force -ErrorAction SilentlyContinue }

Write-Output ''
Write-Output ('  {0,7}  {1,9}  {2,10}  {3,10}  {4,7}' -f 't(s)', 'thread %', 'thread id', 'process %', 'conhost')
foreach ($r in $rows) {
    Write-Output ('  {0,7}  {1,9}  {2,10}  {3,10}  {4,7}' -f $r.Seconds, $r.ThreadPct, $r.HottestId, $r.ProcessPct, $r.Conhost)
}
Write-Output ''

Write-Output ("peak single-thread CPU : {0:N1}% of one core (thread {1}; budget {2}%)" -f $peakThread, $peakThreadId, $MaxPercentOfOneCore)
Write-Output ("peak process CPU       : {0:N1}% of one core" -f $peakTotal)
Write-Output ("new console hosts      : {0} (budget {1})" -f $maxConsoles, $MaxNewConsoles)
Write-Output ("window responding      : {0}" -f $responding)

if ($peakThread -gt $MaxPercentOfOneCore) {
    $failures += ("a single thread held {0:N1}% of one core for {1}s; budget is {2}%" -f $peakThread, $Seconds, $MaxPercentOfOneCore)
}
if ($maxConsoles -gt $MaxNewConsoles) {
    $failures += ("{0} console host(s) appeared while the window ran; budget is {1}" -f $maxConsoles, $MaxNewConsoles)
}
if ($responding -eq $false) {
    $failures += 'the window stopped answering messages'
}
if ($responding -eq 'exited' -and $exitCode) {
    $failures += ("the window exited by itself with code {0}" -f $exitCode)
}

Write-Output ''
if ($failures.Count -eq 0) {
    Write-Output 'PASS'
    exit 0
}
Write-Output 'FAIL'
$failures | ForEach-Object { Write-Output "  - $_" }
exit 1
