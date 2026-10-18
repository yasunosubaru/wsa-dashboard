# SPDX-FileCopyrightText: Copyright (c) 2026
# SPDX-License-Identifier: GPL-3.0-only
<#
.SYNOPSIS
    Fail the build if the window burns CPU, opens console windows, or stops answering.

.DESCRIPTION
    Exists because the screenshot harness could not see any of those. It launches
    the window hidden and photographs one settled frame, so a program that spawns
    a visible console per `adb` call, or that spins a core at 80%, produces a
    perfect picture of a broken application. Both defects shipped in v0.1.0.

    **Repeats, not one sample.** Measured on this machine: the same binary, same
    session, one minute apart, gave 79.3% and 69.6% -- about ten points of drift.
    A gate built on a single window would therefore flap, and a bisection built on
    single windows reports noise as cause (which is exactly what happened while
    chasing the v0.1.0 defect). So each condition runs `-Repeats` times and the
    verdict is on the **median**, with the spread printed next to it: a difference
    smaller than the spread is not a difference.

    Three assertions, each a real defect this project has had:

      * **CPU on one thread.** A GUI program with nothing to do is a few percent of
        one core. Measured per *thread*, because the process total hides which one,
        and a repaint storm and a busy loop then look identical.
      * **No visible console windows charged to this app.** A conhost counts
        only when it has a window (`MainWindowHandle != 0`) AND two-level
        ancestry reaches this process. One transient headless conhost per
        `adb` call is design (`CREATE_NO_WINDOW`, executor.rs), measured
        ~1.1 s apart and `MainWindowHandle=0` every time; it is printed per
        run as information and never charged. Other sessions' consoles are
        excluded by the ancestry check -- the previous whole-machine net
        diff counted their churn as this app's defect (documented:
        one invocation read 1 / 0 / 1 for the same binary).
      * **The window answers.** `Responding` is coarse, but a window that cannot
        answer a message is frozen whatever the CPU says.

.EXAMPLE
    powershell -NoProfile -File tools\cpu-budget.ps1
    powershell -NoProfile -File tools\cpu-budget.ps1 -Repeats 5 -Seconds 15 -MaxPercentOfOneCore 10
#>
[CmdletBinding()]
param(
    [string] $Exe = 'target\release\wsadashboard.exe',
    [int] $Seconds = 12,
    [int] $WarmupSeconds = 20,
    [int] $Repeats = 3,
    [double] $MaxPercentOfOneCore = 20,
    [int] $MaxNewConsoles = 0,
    [string] $Adb = '',
    [switch] $SkipClose
)

$ErrorActionPreference = 'Stop'
Set-Location (Split-Path -Parent $PSScriptRoot)

if (-not (Test-Path $Exe)) { throw "missing $Exe; run build.ps1 first" }
$exePath = (Resolve-Path $Exe).Path
if ($Adb) { $adbPath = (Resolve-Path $Adb).Path } else { $adbPath = '' }

function Sample-Conhosts {
    param([hashtable] $Prev)

    $now = @{}
    try {
        Get-CimInstance Win32_Process -Filter "Name='conhost.exe'" -ErrorAction SilentlyContinue |
            ForEach-Object { $now[[uint32]$_.ProcessId] = $_.ParentProcessId }
    } catch { }

    foreach ($c in $now.Keys) {
        if ($Prev.ContainsKey($c)) { continue }
        $mainwin = 0
        try {
            $p = Get-Process -Id $c -ErrorAction SilentlyContinue
            if ($p) { $mainwin = $p.MainWindowHandle.ToInt64() }
        } catch { }
        if ($mainwin -eq 0) {
            # no window at all: headless console host, the documented cost of
            # CREATE_NO_WINDOW children, never user-facing
            $script:headless++
            continue
        }
        # has a window: charge it only if this app opened it
        $mine = $false
        try {
            $parent = Get-CimInstance Win32_Process -Filter "ProcessId=$($Prev[$c])" -ErrorAction SilentlyContinue
            if ($parent) {
                if ([uint32]$parent.ProcessId -eq $script:proc.Id) { $mine = $true }
                else {
                    $gp = Get-CimInstance Win32_Process -Filter ("ProcessId={0}" -f $parent.ParentProcessId) -ErrorAction SilentlyContinue
                    if ($gp -and [uint32]$gp.ProcessId -eq $script:proc.Id) { $mine = $true }
                }
            }
        } catch { }
        if ($mine) { $script:attrib++ }
    }
    return $now
}

function Measure-Once {
    param([string] $Label)

    Get-Process wsadashboard -ErrorAction SilentlyContinue | Stop-Process -Force
    Start-Sleep -Milliseconds 500

    # The whole-machine net diff this replaces sampled conhost counts at two
    # instants and printed the difference as the verdict: MAPPING documented
    # -4/-5/+5 flips caused by neighbours' terminals, and this app's own
    # headless consoles (one transient conhost per `adb` call, ~1.1 s apart,
    # measured: every one of them MainWindowHandle=0) fall into that diff by
    # phase luck -- the demonstrable bottom line was one invocation reading
    # 1 / 0 / 1 for the same binary. The sampler below charges a conhost to
    # this app only when BOTH hold:
    #   (a) a window exists: MainWindowHandle != 0 (the defect this gate is
    #       about is a user-facing console, not a process id), and
    #   (b) two-level ancestry reaches this process (conhost -> adb.exe ->
    #       wsadashboard.exe), so other sessions' consoles stay out.
    # Our own headless churn is counted separately ($headless) and printed
    # as information only: it is the CREATE_NO_WINDOW design working as
    # documented in src/wsa/executor.rs.
    Remove-Item env:WSADASH_ADB -ErrorAction SilentlyContinue
    if ($adbPath) { $env:WSADASH_ADB = $adbPath }
    $proc = Start-Process -FilePath $exePath -PassThru
    Remove-Item env:WSADASH_ADB -ErrorAction SilentlyContinue
    $script:proc = $proc
    $script:attrib = 0
    $script:headless = 0

    $framePrev = @{}
    Get-CimInstance Win32_Process -Filter "Name='conhost.exe'" -ErrorAction SilentlyContinue |
        ForEach-Object { $framePrev[[uint32]$_.ProcessId] = $_.ParentProcessId }

    # Past the ramp. The burn does not start until the first poll lands and then
    # climbs for a while, so a window that begins too early measures the climb.
    for ($i = 0; $i -lt ($WarmupSeconds * 2); $i++) {
        Start-Sleep -Milliseconds 500
        $framePrev = Sample-Conhosts $framePrev
    }
    $proc.Refresh()
    if ($proc.HasExited) {
        return [pscustomobject]@{ Label = $Label; Cpu = -1; Consoles = 0; Headless = 0; Responding = $false; SelfExited = $true }
    }

    $t0 = @{}
    foreach ($t in $proc.Threads) { $t0[$t.Id] = $t.TotalProcessorTime.TotalMilliseconds }
    $sw = [Diagnostics.Stopwatch]::StartNew()
    for ($i = 0; $i -lt ($Seconds * 2); $i++) {
        Start-Sleep -Milliseconds 500
        $framePrev = Sample-Conhosts $framePrev
    }
    $proc.Refresh()
    $sw.Stop()

    $hot = 0
    foreach ($t in $proc.Threads) {
        $before = if ($t0.ContainsKey($t.Id)) { $t0[$t.Id] } else { 0 }
        $delta = $t.TotalProcessorTime.TotalMilliseconds - $before
        if ($delta -gt $hot) { $hot = $delta }
    }

    $consoles = $attrib
    $responding = $proc.Responding
    $selfExited = $false
    if (-not $SkipClose) {
        try { $null = $proc.CloseMainWindow() } catch { }
        Start-Sleep -Milliseconds 400
        $proc.Refresh()
        if (-not $proc.HasExited) { $responding = $proc.Responding }
    }
    if (-not $proc.HasExited) { $proc | Stop-Process -Force -ErrorAction SilentlyContinue }
    Start-Sleep -Milliseconds 400

    [pscustomobject]@{
        Label      = $Label
        Cpu        = [Math]::Round(100.0 * $hot / $sw.Elapsed.TotalMilliseconds, 1)
        Consoles   = $consoles
        Headless   = $headless
        Responding = $responding
        SelfExited = $selfExited
    }
}

$runs = @()
for ($i = 1; $i -le $Repeats; $i++) {
    $runs += Measure-Once -Label "run $i"
    Write-Verbose "run $i done"
}

$cpus = @($runs | Where-Object { $_.Cpu -ge 0 } | ForEach-Object { $_.Cpu })
$sorted = @($cpus | Sort-Object)
$n = $sorted.Count
if ($n -eq 0) { $median = -1 }
elseif ($n % 2 -eq 1) { $median = $sorted[($n - 1) / 2] }
else { $median = [Math]::Round(($sorted[$n / 2 - 1] + $sorted[$n / 2]) / 2, 1) }
$min = if ($sorted.Count) { $sorted[0] } else { -1 }
$max = if ($sorted.Count) { $sorted[-1] } else { -1 }
$spread = if ($sorted.Count) { [Math]::Round($max - $min, 1) } else { 0 }
$consoleTotal = ($runs | Measure-Object -Property Consoles -Maximum).Maximum
$exited = @($runs | Where-Object { $_.SelfExited }).Count
$frozen = @($runs | Where-Object { -not $_.SelfExited -and $_.Responding -eq $false }).Count

Write-Output ''
Write-Output ('  {0,-8} {1,10}  {2,9}  {3}' -f 'run', 'cpu %', 'conhost', 'responding')
foreach ($r in $runs) {
    Write-Output ('  {0,-8} {1,10}  {2,9}  {3}' -f $r.Label, $r.Cpu, $r.Consoles, $r.Responding)
}
Write-Output ''
Write-Output ("median {0:N1}%   min {1:N1}%   max {2:N1}%   spread {3:N1} points   (budget {4:N0}%)" -f $median, $min, $max, $spread, $MaxPercentOfOneCore)
Write-Output ("new console hosts, worst run: {0} (budget {1})" -f $consoleTotal, $MaxNewConsoles)
Write-Output ("frozen windows: {0}   runs that exited by themselves: {1}" -f $frozen, $exited)
Write-Output ''
Write-Output "note: a difference smaller than the spread ($spread points) is not a difference; re-run before believing one."
Write-Output ("headless console churn (information only, not charged): {0} across {1} runs" -f (($runs | Measure-Object -Property Headless -Sum).Sum), $Repeats)
Write-Output ''

$failures = @()
if ($median -gt $MaxPercentOfOneCore) {
    $failures += ("median single-thread CPU {0:N1}% exceeds the {1:N0}% budget" -f $median, $MaxPercentOfOneCore)
}
if ($cpus.Count -lt $Repeats) {
    $failures += ("only {0} of {1} runs produced a measurement" -f $cpus.Count, $Repeats)
}
if ($consoleTotal -gt $MaxNewConsoles) {
    $failures += ("{0} visible console window(s) charged to this app; budget is {1}" -f $consoleTotal, $MaxNewConsoles)
}
if ($frozen -gt 0) { $failures += 'a window stopped answering messages' }
if ($exited -gt 0) { $failures += 'a run exited by itself' }

Write-Output ''
if ($failures.Count -eq 0) { Write-Output 'PASS'; exit 0 }
Write-Output 'FAIL'
$failures | ForEach-Object { Write-Output "  - $_" }
exit 1
