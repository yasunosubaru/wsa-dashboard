# SPDX-FileCopyrightText: Copyright (c) 2026
# SPDX-License-Identifier: GPL-3.0-only
<#
.SYNOPSIS
  One screenshot of the home page in one tier.

.DESCRIPTION
  Runs the GUI against a stand-in adb, waits for the poll to settle, then asks
  the window to draw itself into a bitmap and writes a PNG.

  Exists because the tiers that matter -- ready, booting, down -- cannot be
  produced on a real machine at will: WSA is either up or it is not, and
  "mid-boot" is not something a reviewer can wait for on demand. Spec section 5
  is entirely about those tiers, so the evidence for them has to be
  reproducible (MAPPING 7).

  What it takes to photograph a window on Windows is most of the story here,
  and every step of it was found the hard way:

    * `Graphics.CopyFromScreen` photographs the desktop, and on this machine it
      returned frames from *before* the window was moved. Clean pictures, right
      dimensions, no error -- of the desktop as it used to be. Three captures
      came back as completely unrelated applications and the home page came
      back looking like its own first frame while its own model said it had
      loaded. Touching the screen DC first refreshes DWM's redirection surface
      and makes it mostly behave, which is how long this was fought.

    * What is *on top* is a separate problem, and hit-testing does not solve
      it. `WindowFromPoint` reported our own window at every sample point of a
      capture that plainly had a chat window across its top-left corner, and a
      centre-only check passed it. A chat client and a monitoring window on
      this desktop both sit above everything and cannot be raised past.

    * Screen colour management is a third: f.lux runs here, DWM composes
      through it, and every capture path that goes via the screen came back
      #FFA250. The home page is white; an orange page is the screen's doing.

  `PrintWindow` is what ends all three. It asks the *window* to draw itself
  into a memory DC, so nothing on top of it matters, no surface can be stale,
  and no display filter is between the pixels and the file. Everything below
  is small because of that.

.EXAMPLE
  powershell -NoProfile -ExecutionPolicy Bypass -File tools\standin-adb\capture.ps1 -State ready
#>
[CmdletBinding()]
param(
    [ValidateSet('ready', 'booting', 'offline', 'stopped')]
    [string]$State = 'ready',

    [string]$OutFile,

    # Photograph the real subsystem instead of a stand-in.
    #
    # The four states are only reproducible against stand-ins -- "mid-boot" is
    # not something a reviewer can wait for on demand -- but a stand-in proves
    # nothing about the real adb path, and the one thing this script cannot
    # check for itself is that `WSADASH_ADB` is honoured at all. This switch
    # leaves it unset so adb resolves from PATH, and says so in the output
    # rather than quietly producing a file that looks like the other four.
    [switch]$UseRealAdb,

    # Seconds to wait for the first poll. The monitor refreshes immediately
    # on start, but a stand-in that spawns six processes still takes a
    # moment, and a picture of the pre-refresh frame is a picture of the
    # loading overlay, which says nothing about the tier.
    [int]$SettleSeconds = 5,

    # The size the four captures share, in the logical units `app.slint` uses.
    # It is multiplied by the monitor's scale before it is compared with the
    # window, because Slint lays out in logical units and Win32 reports pixels;
    # at 150% an 800x600 window is 1200x900 on disk, and a capture that forgot
    # that took the top two thirds of the page and called it the page.
    [int]$W = 800,
    [int]$H = 600,

    # The home page's flat field: `Theme.content_bg` in light mode, from
    # `src/ui/theme.slint`. In every tier that field reaches the corners, so
    # it is both what a capture's corners must be and what says "this is the
    # page" rather than "this is a blank rectangle". If that token changes,
    # this must change with it.
    [string]$ExpectedBackground = '#FFFFFF',

    # Distinct colours in the content area for the capture to be accepted.
    # Eight is the floor for "this is a UI and not one flat field": a window
    # that has not painted photographs as exactly one colour, and the flattest
    # real page -- the loading overlay, which is the caption, a search box and
    # two lines of text -- measures eight. The floor exists because a blank
    # white image would otherwise be filed as evidence of a tier.
    [int]$MinDiversity = 8
)

$ErrorActionPreference = 'Stop'

$repo = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$exe = Join-Path $repo 'target\release\wsadashboard.exe'
if (-not (Test-Path $exe)) { throw "missing $exe; run build.ps1 first" }
$adb = if ($UseRealAdb) { $null } else { Join-Path $PSScriptRoot "adb-$State.cmd" }
if ($adb -and -not (Test-Path $adb)) { throw "missing stand-in $adb" }
if (-not $OutFile) {
    $shots = Join-Path $repo 'docs\screenshots'
    New-Item -ItemType Directory -Force -Path $shots | Out-Null
    $OutFile = Join-Path $shots $(if ($UseRealAdb) { 'p2-real.png' } else { "p2-$State.png" })
}

Add-Type -AssemblyName System.Drawing
# `System.Drawing` is not in the default reference set, so a type definition
# that mentions `Bitmap` has to ask for it. Without this the whole script dies
# at compile time with a message about a namespace, which says nothing about
# the screenshot it was in the middle of taking.
Add-Type -ReferencedAssemblies System.Drawing -TypeDefinition @"
using System;
using System.Runtime.InteropServices;

/// Make this process per-monitor DPI aware, and do it *first*.
///
/// Everything else here depends on it and nothing says so. A DPI-unaware
/// process is handed *logical* coordinates by Win32 while the desktop is in
/// physical pixels, and this machine runs at 150%, so `GetClientRect` answers
/// 800x600 for a window that is really 1200x900. `PrintWindow` then draws the
/// window's physical pixels into a bitmap sized for the logical ones -- it
/// *crops* rather than scales -- and everything below the bottom 400 logical
/// pixels falls outside the picture.
///
/// That is not a hypothetical. The home page's toast sits at y=534, which is
/// y=801 in the pixels the renderer produced, and it was therefore invisible in
/// every capture: not missing, not transparent, simply outside the frame. The
/// same mismatch is why a window asked for at (300,200) kept turning up at
/// (464,310) in every screenshot -- which looked, for most of a day, like the
/// window refusing to be moved.
///
/// `SetProcessDpiAwarenessContext` has to run before any window or DC is
/// touched, so it cannot be a helper called next to the capture.
public static class Dpi {
  [DllImport("user32.dll")]
  public static extern bool SetProcessDpiAwarenessContext(IntPtr value);
  [DllImport("user32.dll")]
  public static extern IntPtr GetProcessDpiAwarenessContext();
  [DllImport("shcore.dll")]
  public static extern int GetDpiForMonitor(IntPtr monitor, int type, out uint x, out uint y);

  // DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2
  public static void MakeAware() {
    SetProcessDpiAwarenessContext(new IntPtr(-4));
  }
  public static string Describe() {
    uint x, y;
    IntPtr monitor = MonitorFromPoint(new POINT { X = 0, Y = 0 }, 2); // DEFAULTTOPRIMARY
    int hr = GetDpiForMonitor(monitor, 0, out x, out y);              // MDT_EFFECTIVE_DPI
    if (hr != 0) return "GetDpiForMonitor failed: " + hr;
    return "effective DPI " + x + "x" + y + " (" + Scale().ToString("0.##") + "x)";
  }

  /// Logical-to-physical scale factor. 1.5 on this machine, which is the
  /// number every other wrong answer in this script was a multiple of.
  public static double Scale() {
    uint x, y;
    IntPtr monitor = MonitorFromPoint(new POINT { X = 0, Y = 0 }, 2);
    if (GetDpiForMonitor(monitor, 0, out x, out y) != 0) return 1.0;
    return x / 96.0;
  }
  [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X, Y; }
  [DllImport("user32.dll")] public static extern IntPtr MonitorFromPoint(POINT p, uint flags);
}
"@

[Dpi]::MakeAware()
$scale = [Dpi]::Scale()
$expectedW = [int][Math]::Round($W * $scale)
$expectedH = [int][Math]::Round($H * $scale)
Write-Verbose ("capture process is per-monitor aware; " + [Dpi]::Describe() +
               "; expecting a ${W}x${H} window as ${expectedW}x${expectedH} pixels")

Add-Type -ReferencedAssemblies System.Drawing -TypeDefinition @"
using System;
using System.Text;
using System.Drawing;
using System.Drawing.Imaging;
using System.Runtime.InteropServices;

/// Everything this script needs from Win32, in one place.
///
/// The window is *enumerated* rather than taken from
/// `Process.MainWindowHandle`, which on three of four runs answered with this
/// binary's hidden single-instance handshake window -- an untitled 158x26
/// shim -- so the capture framed the shim while the real window sat on top of
/// it. An enumeration with a size floor cannot make that mistake, and needs
/// no knowledge of how many windows the app makes.
public static class Wsa {
  public delegate bool EnumProc(IntPtr h, IntPtr l);

  [StructLayout(LayoutKind.Sequential)]
  public struct RECT { public int Left, Top, Right, Bottom; }

  [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc cb, IntPtr l);
  [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)]
  static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] static extern bool GetClientRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr hdc, uint flags);

  /// The client area's size in *physical* pixels.
  ///
  /// Read, never asked for. `app.slint` declares an 800x600 window, so the app
  /// opens at 800x600 whatever it is asked for, and a capture that forced a
  /// size was doing arithmetic on a caption height and a border width to land
  /// on the number `app.slint` already states. Both numbers come from the
  /// window; at 150% they are 1200x900.
  public static Size ClientSize(IntPtr win) {
    RECT cr;
    GetClientRect(win, out cr);
    return new Size(cr.Right - cr.Left, cr.Bottom - cr.Top);
  }

  /// The largest visible, titled window of a process, at least `minW` x `minH`.
  public static IntPtr UiWindowFor(int pid, int minW, int minH) {
    IntPtr best = IntPtr.Zero;
    long bestArea = -1;
    EnumWindows((h, _) => {
      if (!IsWindowVisible(h)) return true;
      var title = new StringBuilder(256);
      GetWindowText(h, title, 256);
      if (title.Length == 0) return true;
      uint owner;
      GetWindowThreadProcessId(h, out owner);
      if (owner != pid) return true;
      RECT r;
      GetWindowRect(h, out r);
      int w = r.Right - r.Left, ht = r.Bottom - r.Top;
      if (w < minW || ht < minH) return true;
      long area = (long)w * ht;
      if (area > bestArea) { bestArea = area; best = h; }
      return true;
    }, IntPtr.Zero);
    return best;
  }

  /// Ask a window to draw itself.
  ///
  /// `PW_RENDERFULLCONTENT` (0x2) is what makes it work for a window this
  /// renderer owns; without it the answer is an empty bitmap, which looks
  /// exactly like a window that has not painted and is the reason this was
  /// first written off as unusable.
  ///
  /// The bitmap is sized from the *client* rectangle because that is what the
  /// window's own layout fills. Anything else -- `GetWindowRect`, DWM's
  /// extended frame bounds -- brings the caption and the invisible resize
  /// border along, and those are not the page.
  public static Bitmap Self(IntPtr win) {
    RECT cr;
    GetClientRect(win, out cr);
    int w = cr.Right - cr.Left, h = cr.Bottom - cr.Top;
    if (w <= 0 || h <= 0) throw new Exception("the window has no client area");
    var bmp = new Bitmap(w, h, PixelFormat.Format32bppArgb);
    using (var g = Graphics.FromImage(bmp)) {
      IntPtr dc = g.GetHdc();
      try { PrintWindow(win, dc, 2); }
      finally { g.ReleaseHdc(dc); }
    }
    return bmp;
  }
}
"@

# Distinct colours over the content area -- below the search box, which is the
# one element in every tier, so a background-only render cannot count as a UI.
function Get-ContentDiversity($bitmap) {
    $seen = @{}
    for ($y = 90; $y -lt $bitmap.Height - 10; $y += 7) {
        for ($x = 10; $x -lt $bitmap.Width - 10; $x += 7) {
            $c = $bitmap.GetPixel($x, $y)
            $seen[('{0:X2}{1:X2}{2:X2}' -f $c.R, $c.G, $c.B)] = $true
        }
    }
    return $seen.Count
}

# The dominant colour of the picture, which is the page's flat field in every
# tier and something else entirely if the window never painted.
#
# This replaced a check on the corners and the sides, which existed to catch a
# window with something lying over it. `PrintWindow` cannot produce that, so
# the check went with it -- and it was reporting false anyway, because the
# window's own left border is black and no corner of a real capture is the
# colour the page is made of. A check that cannot fail for the reason it was
# written is worse than no check: it would have gone on refusing good captures.
function Get-DominantColour($bitmap) {
    $hist = @{}
    for ($y = 0; $y -lt $bitmap.Height; $y += 4) {
        for ($x = 0; $x -lt $bitmap.Width; $x += 4) {
            $c = $bitmap.GetPixel($x, $y)
            $k = '{0:X2}{1:X2}{2:X2}' -f $c.R, $c.G, $c.B
            if ($hist.ContainsKey($k)) { $hist[$k]++ } else { $hist[$k] = 1 }
        }
    }
    if ($hist.Count -eq 0) { return '#000000' }
    $top = $hist.GetEnumerator() | Sort-Object Value -Descending | Select-Object -First 1
    return ('#{0}' -f $top.Key)
}

# A single instance of the app would take a second launch as "focus the one
# that is already there" (P7 wires that deliberately), so no launch can be
# left running from a previous run.
Get-Process -Name 'wsadashboard' -ErrorAction SilentlyContinue | Stop-Process -Force

$proc = $null
try {
    # `$null` rather than "" -- an empty `WSADASH_ADB` is a path of zero length,
    # and `Command::new("")` fails in a way that looks like a broken install
    # rather than an unset variable.
    $env:WSADASH_ADB = $adb
    $proc = Start-Process -FilePath $exe -PassThru
    if ($UseRealAdb) {
        Write-Output 'note: -UseRealAdb, so this is the real subsystem through adb on PATH.'
        Write-Output '      whatever tier it is in right now is the tier in the file, not a chosen one.'
    }

    $deadline = (Get-Date).AddSeconds(15)
    while ((Get-Date) -lt $deadline) {
        $hWnd = [Wsa]::UiWindowFor($proc.Id, 200, 150)
        if ($hWnd -ne [IntPtr]::Zero) { break }
        Start-Sleep -Milliseconds 200
    }
    if ($hWnd -eq [IntPtr]::Zero) {
        throw "the window never appeared; see $HOME\.wsadashboard\logs\wsadashboard.log"
    }

    # Let it finish waking up: the first frames of a freshly launched Slint
    # window are not the page, and the poll refreshes on start, which is what
    # the stand-in's six subprocesses are being waited for.
    Start-Sleep -Seconds $SettleSeconds

    # The size the four captures share: `app.slint`'s 800x600, in the physical
    # pixels this monitor actually has (1200x900 at 150%). If the window opened
    # at anything else the four are not comparable, so that is refused rather
    # than resized -- an app that remembered a size would otherwise silently
    # produce four pictures that cannot be laid side by side.
    $client = [Wsa]::ClientSize($hWnd)
    if ($client.Width -ne $expectedW -or $client.Height -ne $expectedH) {
        throw ("the window's client area is {0}x{1}, not the {2}x{3} that app.slint declares at this " +
               "monitor's scale. The four captures have to be the same shape to be worth comparing, " +
               "and resizing the window is not this script's business.") -f `
               $client.Width, $client.Height, $expectedW, $expectedH
    }

    $bitmap = $null
    $best = 0
    $dominant = ''
    foreach ($attempt in 1..8) {
        $cand = [Wsa]::Self($hWnd)
        $diversity = Get-ContentDiversity $cand
        if ($diversity -gt $best) { $best = $diversity }
        if ($diversity -lt $MinDiversity) {
            $cand.Dispose()
            Start-Sleep -Milliseconds 800
            continue
        }
        $dominant = Get-DominantColour $cand
        if ($dominant.ToUpperInvariant() -ne $ExpectedBackground.ToUpperInvariant()) {
            # The window has something in it that is not the page -- which,
            # now that nothing can lie over the window, means it is showing
            # something other than the home page at all.
            $cand.Dispose()
            Start-Sleep -Milliseconds 800
            continue
        }
        $bitmap = $cand
        break
    }

    if (-not $bitmap) {
        if ($dominant -and $dominant.ToUpperInvariant() -ne $ExpectedBackground.ToUpperInvariant()) {
            throw ('the window is not showing the home page: its largest area is ' + $dominant +
                   ', not the flat field of Theme.content_bg (' + $ExpectedBackground + '). ' +
                   "See $HOME\.wsadashboard\logs\wsadashboard.log for the poll's own account of itself.")
        }
        throw ('the content area never rendered as more than a flat field (best of eight was ' +
               "$best distinct colours, floor was $MinDiversity); see " +
               "$HOME\.wsadashboard\logs\wsadashboard.log")
    }

    try { $bitmap.Save($OutFile, [System.Drawing.Imaging.ImageFormat]::Png) }
    finally { $bitmap.Dispose() }

    Write-Output "state=$State -> $OutFile  ($($client.Width)x$($client.Height))"
}
finally {
    # Null-safe: a refusal before the launch leaves $proc unset, and a guard
    # that dereferenced it would mask the original error.
    if ($proc -and -not $proc.HasExited) { $proc.Kill() }
    $env:WSADASH_ADB = $null
}
