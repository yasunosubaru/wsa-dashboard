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

    # Seconds to wait *after* the window appears, if waiting for the app's own
    # Seconds to wait *after* the model arrives, for the paint to land.
    #
    # The wait for the model itself is not a countdown -- it is the app saying
    # out loud that it has built a view model and pushed it at the window
    # (`home model applied`). Guessing instead produced five empty captures in
    # a row: the stand-in's first poll lands about six seconds after launch, the
    # fixed sleep was five, and the page's own diversity check could not tell
    # "shell only" from "shell plus list", because the shell alone is already
    # more than eight colours. If the line never appears this falls back to the
    # sleep alone rather than hanging, and says so.
    [int]$SettleSeconds = 2,
    [int]$ModelTimeoutSeconds = 45,
    # Where the app writes. Fixed because the app's directory is fixed
    # (spec section 6). This script needs the file's *size before launch* to tell
    # a new line from one left by an earlier run.
    [string]$LogDir = "$HOME\.wsadashboard\logs",

    # The size the four captures share, in the logical units `app.slint` uses.
    # It is multiplied by the monitor's scale before it is compared with the
    # window, because Slint lays out in logical units and Win32 reports pixels;
    # at 150% an 800x600 window is 1200x900 on disk, and a capture that forgot
    # that took the top two thirds of the page and called it the page.
    [int]$W = 800,
    [int]$H = 600,

    # Two more states of the shell, reached by clicking the shell itself.
    #
    # The side bar's collapse and the theme are `in-out` properties with no
    # persistence until P6/P8, so there is no file to pre-set and no command
    # line switch to hand -- the only way to reach them is what a user does.
    # The coordinates are *derived* from the layout rather than typed in:
    # `Sidebar` is 54px collapsed with its hamburger centred in a 36px row below
    # the title bar's 40px, and `TitleBar`'s controls are four 46px buttons
    # right-aligned with the 40px switch first. They are multiplied by the
    # monitor scale here for the same reason every other number in this script
    # is.
    #
    # A click that lands on nothing produces a picture that looks like the
    # default state, so each of these is read back from the image it writes
    # rather than trusted: the collapsed capture must show a 81px-wide bar
    # (54px at 1.5) and the dark capture must not have #FFFFFF as its dominant
    # colour.
    # Steps to click through before shooting, comma-separated or repeated.
    #
    # A single `string` rather than an array, because arrays do not survive
    # `powershell -File`: the parent's own shell parses `go-device-page,
    # device-expand` into two arguments, but a command line handed to another
    # process is just text, so the child saw the whole comma-joined token and
    # refused it against the allowed list. Splitting here means both spellings
    # work -- `-Act a,b` and `-Act a -Act b` -- and an unknown name is caught
    # by the switch below, which is where it can say what the valid ones are.
    [string]$Act = 'none',

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
    [int]$MinDiversity = 8,

    # Pause the screen's colour filter for the capture and restart it after.
    #
    # f.lux composes through DWM over the whole desktop, so any capture that
    # reads the screen is tinted -- white comes back as something else, and how
    # much else changes with the hour. Without this the script refuses the
    # picture and names the filter, which is the right answer when the caller
    # can simply turn it off; with it the filter is paused for one capture and
    # restored in the `finally`, so nothing is left switched off.
    [switch]$BypassDisplayFilter
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
/// physical pixels, and this machine runs at 150%, so 'GetClientRect' answers
/// 800x600 for a window that is really 1200x900. 'PrintWindow' then draws the
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
/// 'SetProcessDpiAwarenessContext' has to run before any window or DC is
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
/// 'Process.MainWindowHandle', which on three of four runs answered with this
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
  [DllImport("user32.dll", CharSet=CharSet.Unicode)]
  static extern int GetClassName(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] static extern bool GetClientRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool SetWindowPos(
      IntPtr h, IntPtr insertAfter, int x, int y, int w, int ht, uint flags);
  [DllImport("user32.dll")] static extern bool BringWindowToTop(IntPtr h);
  [DllImport("user32.dll")] static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] static extern void mouse_event(uint flags, int dx, int dy, uint data, IntPtr extra);

  // The screen side: what is *in front of* the window, and a current surface to
  // read it from. 'GetDC'/'ReleaseDC' are the warm-up described on
  // 'WarmDesktopSurface' below; 'WindowFromPoint' is the ownership question.
  [DllImport("user32.dll")] static extern IntPtr GetDC(IntPtr h);
  [DllImport("user32.dll")] static extern int ReleaseDC(IntPtr h, IntPtr hdc);
  [DllImport("user32.dll")] static extern IntPtr WindowFromPoint(POINT p);
  [DllImport("user32.dll")] static extern IntPtr GetAncestor(IntPtr h, uint flags);

  [StructLayout(LayoutKind.Sequential)]
  public struct POINT { public int X, Y; }

  /// The client area's size in *physical* pixels.
  ///
  /// Read, never asked for. 'app.slint' declares an 800x600 window, so the app
  /// opens at 800x600 whatever it is asked for, and a capture that forced a
  /// size was doing arithmetic on a caption height and a border width to land
  /// on the number 'app.slint' already states. Both numbers come from the
  /// window; at 150% they are 1200x900.
  public static Size ClientSize(IntPtr win) {
    RECT cr;
    GetClientRect(win, out cr);
    return new Size(cr.Right - cr.Left, cr.Bottom - cr.Top);
  }

  /// The largest visible, titled window of a process, at least 'minW' x 'minH'.
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

  /// What a window is called, for an error message that names the culprit.
  ///
  /// A refusal that says "another window" and nothing else sends the reader to
  /// hunt for it; "another window will not stay behind ours: <title> at (x,y)"
  /// does not.
  public static string Describe(IntPtr h) {
    if (h == IntPtr.Zero) return "the desktop itself";
    var t = new StringBuilder(256);
    GetWindowText(h, t, 256);
    var c = new StringBuilder(256);
    GetClassName(h, c, 256);
    if (t.Length == 0) return "a titleless window of class " + c;
    return "'" + t + "' (" + c + ")";
  }

  // 'WindowFromPoint(POINT)' and 'GetAncestor' were already declared above for
  // 'Describe'; a second 'DllImport' of either with the same signature is a
  // compile error rather than a harmless repeat, so this uses those.
  //
  // Single quotes throughout this block's comments, matching the rest of the
  // class: this is C# inside a PowerShell *double-quoted* here-string, where a
  // backtick is an escape. Backtick followed by the letter n becomes a newline,
  // which is how a comment naming no-frame silently split in two and left the
  // compiler reading "o-frame" as a label. This comment says that without
  // writing the sequence, because writing it here would trigger it.
  static IntPtr At(int x, int y) {
    POINT p; p.X = x; p.Y = y;
    return GetAncestor(WindowFromPoint(p), 2 /* GA_ROOT */);
  }

  /// Whether *our* window is what owns a screen point right now.
  ///
  /// Used to decide how to click. A point that reads correctly inside the
  /// captured image says nothing about who would take the click: 'PrintWindow'
  /// paints the window itself and so is immune to what is stacked above it,
  /// while input goes to whatever is on top. On this desktop something is
  /// routinely on top -- measured, both the sidebar and the tab reported
  /// WindowFromPoint -> 'reader | OpenChamber' (Chrome_WidgetWin_1) at the
  /// moment of the press, and the tab clicks were lost to it while the sidebar
  /// ones happened to land.
  public static bool PointIsOn(IntPtr h, int x, int y) { return At(x, y) == h; }

  [DllImport("user32.dll")]
  static extern bool PostMessage(IntPtr h, uint msg, IntPtr wParam, IntPtr lParam);

  /// Press and release at a point *inside our window*, addressed to the window
  /// rather than to the screen.
  ///
  /// The fallback for a point another window owns: 'PostMessage' puts the mouse
  /// events in our own queue, so the Z-order of unrelated windows stops being a
  /// variable in a screenshot. Coordinates are client pixels, which for a
  /// 'no-frame' window are also the logical layout units multiplied by the
  /// monitor scale -- 'Frame' and 'GetClientRect' coincide by construction (see
  /// 'Frame').
  ///
  /// 'WM_MOUSEMOVE' first for the reason 'ClickAt' moves the cursor first: a
  /// TouchArea that never sees a move has no hover, and a press arriving as the
  /// first message it gets is a press on a button that was never there.
  public static void ClickClient(IntPtr h, int x, int y) {
    IntPtr lp = (IntPtr)((y << 16) | (x & 0xFFFF));
    PostMessage(h, 0x0200 /* WM_MOUSEMOVE */, IntPtr.Zero, lp);
    System.Threading.Thread.Sleep(40);
    PostMessage(h, 0x0201 /* WM_LBUTTONDOWN */, new IntPtr(1), lp);
    System.Threading.Thread.Sleep(40);
    PostMessage(h, 0x0202 /* WM_LBUTTONUP */, IntPtr.Zero, lp);
  }

  /// Click at a point, in physical pixels.
  ///
  /// Move, press, release -- not `mouse_event` alone, because a button that the
  /// cursor never arrived at is not a button anyone clicked, and a `TouchArea`
  /// reading `has-hover` before `pressed` needs the move to have happened.
  public static void ClickAt(int x, int y) {
    SetCursorPos(x, y);
    mouse_event(0x0002, 0, 0, 0, IntPtr.Zero); // left down
    System.Threading.Thread.Sleep(40);
    mouse_event(0x0004, 0, 0, 0, IntPtr.Zero); // left up
  }

  /// Park the cursor somewhere it cannot be hovering anything.
  ///
  /// The same `SetCursorPos` `ClickAt` uses, without the press. A screenshot
  /// is taken at the end of a click sequence, and the cursor is still where
  /// the last click was made -- on a button -- so the image carries that
  /// button's tooltip. Nothing about the app changed.
  public static void MoveAway(int x, int y) {
    SetCursorPos(x, y);
  }

  /// Bring this window above whatever is on the desktop.
  ///
  /// The check below asks five points whether ours is on top, and it is only a
  /// fair question once we have *asked* for that. `SetForegroundWindow` on its
  /// own is routinely refused on Windows -- the foreground lock decides who may
  /// take it -- so the order matters: raise to topmost, then foreground, then
  /// read. Raising is a request rather than a result, which is exactly what the
  /// check is for.
  public static void Raise(IntPtr h) {
    const uint SWP_NOMOVE = 0x0002;
    const uint SWP_NOSIZE = 0x0001;
    SetWindowPos(h, new IntPtr(-1 /* HWND_TOPMOST */), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE);
    BringWindowToTop(h);
    SetForegroundWindow(h);
  }

  /// The window's rectangle on the screen, in physical pixels.
  ///
  /// 'GetWindowRect' rather than 'GetClientRect': with 'no-frame: true' there is
  /// no caption and no invisible resize border, so the two coincide, and the
  /// screen capture below needs the *screen* coordinates of the window.
  public static RECT Frame(IntPtr win) {
    RECT r;
    GetWindowRect(win, out r);
    return r;
  }

  /// Ask DWM for a current frame of the desktop.
  ///
  /// Without this 'Graphics.CopyFromScreen' returns a frame from *before* the
  /// window was last painted -- a clean, correctly sized picture of the desktop
  /// as it used to be. Touching the screen DC forces the redirection surface to
  /// be rebuilt. Nothing about the result distinguishes a stale frame from a
  /// fresh one, which is how three captures ended up being pictures of other
  /// applications while every check in this script passed.
  public static void WarmDesktopSurface() {
    IntPtr dc = GetDC(IntPtr.Zero);
    ReleaseDC(IntPtr.Zero, dc);
  }

  /// The top-level window the mouse would hit at a point.
  public static IntPtr RootAt(int x, int y) {
    var p = new POINT { X = x, Y = y };
    return GetAncestor(WindowFromPoint(p), 2 /* GA_ROOT */);
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

# --- the display filter (f.lux) ---------------------------------------------
#
# f.lux composes through DWM over the whole desktop, so *every* capture path
# that reads the screen comes back tinted -- the app's white flat field reads
# as #FFA250 at night and as nothing at all at midday, which is why the tint
# appears and disappears between runs of the same script.
#
# The filter is paused for the length of one capture and restarted afterwards.
# Pausing is `Stop-Process`, and restarting cannot be the exe -- WindowsApps
# ACLs refuse to launch it directly -- so it goes through the shell's own
# AppsFolder protocol with the AUMID, which is how a Store app is started
# from a command line.
$script:FilterAumid = ''
$script:FilterSuspended = $false

function Get-DisplayFilterAumid {
    $exe = Get-Process -Name 'flux' -ErrorAction SilentlyContinue |
        Select-Object -First 1 -ExpandProperty Path
    if (-not $exe) { return '' }
    # C:\Program Files\WindowsApps\F.luxSoftwareLLC.f.lux_4.137.0.0_x86__sw1dyjdkns7gt\Flux\flux.exe
    #  ^ the folder is <package name>_<version>_<arch>__<publisher id>, and the
    #    family name is the package name plus the publisher, joined by "_".
    $folder = Split-Path -Parent (Split-Path -Parent $exe)
    $leaf = Split-Path -Leaf $folder
    $name = $leaf -replace '_[^_]+_[^_]+__[^_]+$', ''
    $publisher = ($leaf -split '__')[-1]
    if (-not $name -or $publisher -eq $leaf) { return '' }
    return "${name}_${publisher}!f.lux"
}

function Suspend-DisplayFilter {
    $process = Get-Process -Name 'flux' -ErrorAction SilentlyContinue
    if (-not $process) { return }
    $script:FilterAumid = Get-DisplayFilterAumid
    $process | Stop-Process -Force
    $script:FilterSuspended = $true
    # DWM drops the transform as soon as the process dies, so the wait below is
    # a real one rather than a hope -- the check that settles it is the next
    # capture, which is verified rather than trusted.
    Start-Sleep -Seconds 2
}

function Restore-DisplayFilter {
    if (-not $script:FilterSuspended) { return }
    if (-not $script:FilterAumid) {
        # Suspended but with no AUMID means the filter was stopped by a path
        # that gave us nothing to restart -- and a screen left untinted with no
        # way back is worse than a tinted capture, so say it rather than go.
        Write-Output ('the display filter is paused and its AUMID could not be derived; ' +
                      'start it again by hand (' + $script:FilterAumid + ' is empty).')
        return
    }
    Start-Process explorer.exe -ArgumentList "shell:AppsFolder\$($script:FilterAumid)"
    $script:FilterSuspended = $false
    # Said rather than assumed: the process takes a few seconds to appear, so
    # this line -- not the process list -- is the thing a reader can check on.
    Write-Output "display filter restarted ($($script:FilterAumid))"
}

# Is our window the one that is actually there?
#
# Asked of five points, corners included: a check that only looks at the middle
# of the frame passes a window with a chat client lying across one corner of it,
# which is what happened four captures in a row on this desktop.
#
# It was wrong for a while even after being written, and the reason is the same
# one that made every capture the wrong shape: `WindowFromPoint` answers in
# *logical* coordinates to a DPI-unaware process while `CopyFromScreen` reads
# physical pixels, so the question was being asked at a different place from the
# one being photographed. `MakeAware()` runs before any of this, and it is what
# makes the two agree.
#
# Returns the point that answered for something else, or nothing.
function Get-ForeignPoint([IntPtr]$hWnd, $frame) {
    $inset = 12
    $points = @(
        @{ x = $frame.Left + $inset; y = $frame.Top + $inset },
        @{ x = $frame.Right - $inset - 1; y = $frame.Top + $inset },
        @{ x = $frame.Left + $inset; y = $frame.Bottom - $inset - 1 },
        @{ x = $frame.Right - $inset - 1; y = $frame.Bottom - $inset - 1 },
        @{ x = [int](($frame.Left + $frame.Right) / 2); y = [int](($frame.Top + $frame.Bottom) / 2) }
    )
    foreach ($p in $points) {
        $under = [Wsa]::RootAt($p.x, $p.y)
        if ($under -ne $hWnd) {
            $other = 'a window with no title'
            if ($under -ne [IntPtr]::Zero) {
                $other = [Wsa]::Describe($under)
            }
            return ('{0} at ({1},{2})' -f $other, $p.x, $p.y)
        }
    }
    return ''
}

# Wait until the app says it has a view model.
#
# Not a countdown. The app writes `home model applied` at the moment the poll
# has built a model and handed it to the window, which is the only thing worth
# waiting for -- and waiting for it *by name* is what makes the wait the same
# length regardless of how many processes the stand-in has to spawn or how long
# adb takes to answer. The capture below is judged on its pixels afterwards, so
# this is not load-bearing; it exists because guessing the number produced five
# empty pictures that passed every pixel check the script had.
#
# The log line needs debug logging, so the launch sets `RUST_LOG` for the child.
# If it never appears -- the app was built without the line, or the file is not
# where it usually is -- this says so and moves on rather than hanging; the
# pixel checks below are what will refuse the picture if the wait was wrong.
function Wait-ForModel([string]$dir, [int]$timeoutSeconds, [int]$paintSeconds) {
    $log = Get-ChildItem $dir -Filter 'wsadashboard.log.*' -ErrorAction SilentlyContinue |
        Sort-Object LastWriteTime -Descending | Select-Object -First 1
    $before = if ($log) { $log.Length } else { 0 }

    $deadline = (Get-Date).AddSeconds($timeoutSeconds)
    $sawModel = $false
    while ((Get-Date) -lt $deadline) {
        Start-Sleep -Milliseconds 400
        $log = Get-ChildItem $dir -Filter 'wsadashboard.log.*' -ErrorAction SilentlyContinue |
            Sort-Object LastWriteTime -Descending | Select-Object -First 1
        if (-not $log) { continue }
        $stream = [System.IO.File]::Open($log.FullName, 'Open', 'Read', 'ReadWrite')
        try {
            $stream.Position = [Math]::Min($before, $stream.Length)
            $reader = New-Object System.IO.StreamReader($stream)
            $tail = $reader.ReadToEnd()
            $reader.Dispose()
        } finally {
            $stream.Close()
        }
        if ($tail -match 'home model applied') { $sawModel = $true; break }
    }

    if ($sawModel) {
        Write-Verbose 'the app reports it has a view model'
    } else {
        Write-Output ("note: no 'home model applied' within {0}s; falling back to a {1}s sleep." -f `
                      $timeoutSeconds, $paintSeconds)
    }
    Start-Sleep -Seconds $paintSeconds
}

# Reading the log back after a click, which is the only way a swallowed click is
# detectable at all.
#
# It happens: `Raise` asks Windows to bring the window forward and a click
# arriving before that transition completes is consumed by it -- the window
# activates and nothing else does. The 250ms wait below was chosen because it
# usually makes that go away, and "usually" is the problem: it did not on one
# two-step run, so `go-network-page` produced no `tab selected` line, the second
# step clicked the coordinate it would have had if the first had worked, and it
# landed in the home page's search box. The picture that came back was a
# perfectly good photograph of the wrong page -- which is worse than a script
# that failed, because it is evidence.
#
# So a `go-*` step is read back rather than trusted, the same way this script
# already reads back its collapsed and dark captures.
function LogLength([string]$dir) {
    $log = Get-ChildItem $dir -Filter 'wsadashboard.log.*' -ErrorAction SilentlyContinue |
        Sort-Object LastWriteTime -Descending | Select-Object -First 1
    if ($log) { $log.Length } else { 0 }
}

function LogGained([string]$dir, [long]$since, [string]$pattern) {
    $log = Get-ChildItem $dir -Filter 'wsadashboard.log.*' -ErrorAction SilentlyContinue |
        Sort-Object LastWriteTime -Descending | Select-Object -First 1
    if (-not $log) { return $false }
    $stream = [System.IO.File]::Open($log.FullName, 'Open', 'Read', 'ReadWrite')
    try {
        $stream.Position = [Math]::Min($since, $stream.Length)
        $reader = New-Object System.IO.StreamReader($stream)
        $tail = $reader.ReadToEnd()
        $reader.Dispose()
    } finally {
        $stream.Close()
    }
    return ($tail -match $pattern)
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
    # Only for the child, and only so that `Wait-ForModel` has a line to read.
    # This script's own diagnostics are `Write-Verbose`; the child's level is
    # not otherwise its business, and it is put back below.
    $env:RUST_LOG = 'wsadashboard=debug'
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

    # Wait for the app's own signal, then for the paint to land.
    Wait-ForModel $LogDir $ModelTimeoutSeconds $SettleSeconds

    # `-Act` is a sequence, because some states are only reachable by a second
    # click: the device page exists behind the side bar, and its properties
    # table exists behind that page's chevron. One step per run would make
    # either one unshootable.
    $steps = @($Act -split ',') | ForEach-Object { $_.Trim() } | Where-Object { $_ -and $_ -ne 'none' }
    foreach ($step in $steps) {
        if ($step -eq 'none') { continue }
        # Logical coordinates first, physical after -- the window's own layout
        # is in the units `app.slint` declares, and the mouse is in the units the
        # screen is drawn in. Derived from the layout rather than typed in:
        #   collapse  40px title + 12px padding + 4px spacer + half an icon
        #   theme     four 46px buttons right-aligned, the 40px switch first,
        #             so 800 - 46*3 - 20
        #   devices   that, plus one 36px row and its 4px gap
        #   expand    the card sits in the content area's 16px/6px padding and
        #             the list's 4px/16px, its header is 64px tall, and the
        #             chevron is the last 28px of the row minus 8px of card
        $actWhere = switch ($step) {
            'collapse-sidebar' { @{ x = 27; y = 74 } }
            'dark-theme'       { @{ x = 642; y = 20 } }
            # The install page opened a row between the hamburger and the
            # device entry this phase, so both moved: 114 for Home, +40 for
            # install, +40 again for device. The numbers come from the layout
            # rather than being measured off a picture, which is exactly why
            # they have to be recomputed when a row is inserted -- copying the
            # old value would have silently clicked the next entry down, which
            # is what happened.
            'go-install-page'  { @{ x = 27; y = 154 } }
            'go-device-page'   { @{ x = 27; y = 194 } }
            # P5b turned on `show_network`, which sits between device and
            # storage in `sidebar.slint` (line 132, then 142 for scheduler, then
            # 152 for mount), so the network row takes 194 + 40 = 234 and
            # **storage moves down with it**: 274. The scheduler row is still off,
            # which is why it does not add a row of its own.
            #
            # Both numbers change in this one edit on purpose, and the comment on
            # `go-install-page` says why: the rows below an inserted one shift,
            # and updating only the new step would have made `go-storage-page`
            # land on network -- a click that succeeded, a picture that looks
            # plausible, and a storage capture filed under the wrong page.
            'go-network-page'  { @{ x = 27; y = 234 } }
            'go-storage-page'  { @{ x = 27; y = 274 } }
            # The settings page (P6). Measured, not derived -- and the
            # derivation was wrong twice: 540 (rows counted off a picture
            # taken while `Donate` was still on screen) and then 488 (guessing
            # the 40 that hiding it would take). The real answer is 566, from
            # a capture taken after `handlers::settings` started gating each
            # entry on `tabs::has_a_view` -- which removed donate, about *and*
            # scheduler, so the bottom group is one row and neither guess had
            # anything to do with it.
            #
            # Measuring is the rule here rather than the exception: this row is
            # pinned to the bottom by a stretch spacer, so its position depends on
            # how many rows are above it AND on the window height. Only the
            # picture knows either. The `tab selected` read-back is what turns a
            # stale coordinate into a failure instead of a plausible-looking
            # picture of the wrong page.
            'go-settings-page' { @{ x = 27; y = 526 } }
            # The about page (P6, Task 5). Adding a row to the bottom group pushes
            # the rows *above* it up, not the rows below it down: the group is
            # anchored to the bottom of the sidebar, so settings moved 566 -> 526
            # and about took 566. Writing it the other way round -- "one row lower
            # than settings" -- is what this comment originally said, and it landed
            # the click 40px off the bottom of the window.
            #
            # This is the fourth time this pair of coordinates has moved in P6 and
            # the third time the reason was a row appearing or disappearing rather
            # than a layout change. Anything that adds a side-bar entry moves both
            # numbers, and there are two more entries to come (scheduler in P7,
            # donate whenever it is assigned).
            'go-about-page'      { @{ x = 27; y = 566 } }
            # The settings page's three tabs. Measured off the P6 capture
            # (1000x750 at this monitor's 1.25 scale): the tab row's boxes read at
            # General (275..360, 70..113), Advanced (375..475, 70..113) and
            # Interface (483..580, 70..113), which is logical (220, 61), (340, 61)
            # and (424, 61).
            #
            # They are three steps rather than one because the point of the three
            # pictures is that each tab says something different, and a single
            # capture of whichever tab happened to be showing would not be a
            # record of any of them.
            'settings-tab-advanced'  { @{ x = 340; y = 61 } }
            # The page's Save button. Measured off p6-settings-general.png: it
            # reads at (874..971, 685..723), which is (699, 74) at 1.25 scale... see
            # the note below; the numbers are (737, 563) and they come from the same
            # picture. It is in `$setsState` because pressing it twice is
            # harmless -- the second write says the same thing the first did.
            'settings-save'      { @{ x = 737; y = 563 } }
            # The language picker, and one of its rows. The combo box on the
            # General tab reads at (275..615, 148..193) -> logical (356, 137).
            'settings-language'  { @{ x = 356; y = 137 } }
            # One row of that dialog. `de` rather than a Chinese one because it is
            # on the first screen of the picker (the list is sorted, so `zh-CN`
            # needs a scroll) -- the point of the step is that picking a row
            # re-resolves the whole window, and `de` proves that just as well as
            # `zh-CN` would. Read off lang-picker.png: `de` reads at
            # (409..584, 376..430), centre (496, 403), which is (397, 322) at this
            # monitor's 1.25 scale.
            'settings-language-de' { @{ x = 397; y = 322 } }
            # UNVERIFIED. The click before this one is the coordinate the dialog's
            # `de` cell had when the picker opened over a page at 1000x750, and the
            # picker's rows move with `Flickable.viewport-y`. On a run where the
            # monitor's reported scale came back 150% rather than 125% the whole
            # window is 1.2x the size these numbers assume, the click landed on the
            # mask, and the dialog closed without selecting anything -- which looks
            # exactly like a language switch that does nothing.
            #
            # It is left in rather than deleted because the row *is* there and the
            # dialog *does* list all fifty packs with the current one highlighted;
            # what is unproven is that this coordinate reaches it. Re-measure at
            # whatever scale a run reports, then delete this paragraph.
            'settings-tab-interface' { @{ x = 424; y = 61 } }
            # The network page's second tab. Measured off
            # p5b-network-port-forwarding.png (1200x900) rather than derived:
            # the tab bar is inside `NetworkView`'s own 40px row with 8px/4px
            # padding, and the first tab's width is its label's, so its position
            # depends on the font rather than on a layout constant. The "HTTP
            # Proxy" box reads at (504..587, 87..124), centre (546, 106), which
            # is (364, 71) at this monitor's 1.5 scale.
            #
            # Re-measure if the font or the tab labels change -- a tab click that
            # lands a few pixels left of the target does not throw, it leaves the
            # first tab showing and files the second one's picture as the first.
            'network-proxy-tab' { @{ x = 396; y = 71 } }
            # 内容区里唯一有可见效果的控件，用来判定点击到底通不通。图片 1200x900 里垃圾桶在
            # (1131,174) -> 逻辑 (754,116)。删了可复原（端口已知）。
            'network-delete-first' { @{ x = 754; y = 116 } }
            # The page's primary button. Image 1200x900: 'Add Rule' spans about
            # x 1026..1168, y 828..864, centre (1097, 846) -> logical (731, 564).
            'network-add-rule'      { @{ x = 731; y = 564 } }
            # The home page's first card's chevron. Measured off a 1200x900
            # capture of the `ready` stand-in rather than derived, because the
            # home list sits under a search field whose height is set by the
            # font and not by a layout constant: the chevron reads at (1140, 186)
            # in the image, which is (760, 124) at this monitor's 1.5 scale.
            'home-expand'      { @{ x = 760; y = 124 } }
            # The three buttons spec Q22 keeps in the expanded strip, measured
            # off p5a-home-expanded.png (1200x900): the info circle reads at
            # (599, 257), the trash at (997, 257) and the gear at (1063, 257),
            # which is (399, 171), (665, 171) and (709, 171) at 1.5. They move
            # together, so they are written as one number each rather than
            # derived from a shared origin that a future layout change would
            # invalidate silently.
            'home-info'        { @{ x = 399; y = 171 } }
            'home-delete'      { @{ x = 665; y = 171 } }
            'home-settings'    { @{ x = 709; y = 171 } }
            'device-expand'    { @{ x = 756; y = 80 } }
            default {
                throw ("no coordinates known for -Act '{0}'; add it beside the others in " +
                       $PSCommandPath) -f $step
            }
        }
        $before = [Wsa]::Frame($hWnd)
        $logSince = LogLength $LogDir
        # A click that arrives while the window is still being brought forward is
        # consumed by that transition: the window activates and nothing else
        # happens. The 250ms wait makes it rare, and "rare" is not good enough --
        # it cost one two-step run, where `go-network-page` produced no `tab
        # selected` line at all, the second step clicked the coordinate it would
        # have needed had the first worked, and it landed in the home page's
        # search box. The picture that came back was a perfect photograph of the
        # wrong page: worse than a failure, because it reads as evidence.
        #
        # So steps are repeated, and which ones *may* be is a question of whether
        # the action sets state or toggles it:
        #
        #   sets   `go-*` (select a page), `network-proxy-tab` (select a tab).
        #          Clicking three times ends in the same state as once, so a
        #          swallowed first click costs nothing but a few hundred ms.
        #   toggles `collapse-sidebar`, `dark-theme`, and the chevrons, which
        #          *open* a panel that is already open if the first click
        #          worked. Clicking those again would silently produce the
        #          opposite of what was asked, so they get exactly one attempt
        #          and the wait that was already there.
        #
        # `go-*` additionally has its effect read back from the app's log,
        # because it is the only family whose failure produces a *plausible*
        # wrong picture. A second failure there is raised rather than written to
        # disk as evidence.
        $setsState = ($step -like 'go-*') -or ($step -eq 'network-proxy-tab') -or ($step -eq 'network-delete-first') -or ($step -eq 'settings-save')
            # `settings-language-de` is deliberately NOT in the list. It opens the
            # picker and then has to click a row inside it, and the coordinate that
            # depends on the picker having finished scrolling -- which it may not
            # have. A step that cannot be repeated safely belongs to a human, not
            # to a run whose whole purpose is being repeatable; see the note on
            # its own definition below.
        $attempts = if ($setsState) { 3 } else { 1 }
        $applied = $false
        for ($try = 0; $try -lt $attempts -and -not $applied; $try++) {
            [Wsa]::Raise($hWnd)
            # 600ms, up from 250ms, because 250ms was demonstrably not enough:
            # one run failed the *first* sidebar click three times in a row, and
            # three separate runs lost the first click of the sequence -- enough
            # that a run either threw or photographed a page it never opened.
            # The wait is the only thing standing between "the window was asked
            # to come forward" and "the window is forward"; nothing else in this
            # script can tell, since a swallowed click produces no error and no
            # log line of its own.
            Start-Sleep -Milliseconds 600

            # Re-read the frame here rather than reusing the one taken at the
            # top of the step.
            #
            # Windows moves this window between the two: a launch cascades it,
            # and the position differs run to run -- measured across four runs
            # it started at (113,114), (37,38), (265,266) and (151,152). A frame
            # read a moment earlier can therefore be stale by enough to miss the
            # target entirely, and the miss is silent: the click lands on
            # whatever sits at the old coordinates, which is how one run meant
            # for the Network entry selected Storage instead -- one row down,
            # and a picture that looked like a successful navigation.
            $now = [Wsa]::Frame($hWnd)
            if ($now.Left -ne $before.Left -or $now.Top -ne $before.Top) {
                Write-Verbose ("  window moved since the step began ({0},{1}) -> ({2},{3})" -f `
                               $before.Left, $before.Top, $now.Left, $now.Top)
            }
            $before = $now
            $px = [int]($before.Left + ($actWhere.x * $scale))
            $py = [int]($before.Top + ($actWhere.y * $scale))
            # Printed after the re-read, so the number in the log is the one the
            # press actually uses. Printed once, because three attempts print
            # three identical lines and say nothing more.
            if ($try -eq 0) {
                Write-Verbose ("  clicking '{0}' at physical ({1},{2})" -f $step, $px, $py)
            }

            # Who is on that pixel *now*, after the raise and before the press.
            # Not decoration: `PrintWindow` photographs the window regardless of
            # what stacks above it, so a capture can look perfect while every
            # input aimed at that spot goes elsewhere. Where another window owns
            # the point, the click is addressed to our window's queue instead --
            # otherwise the Z-order of an unrelated program is what decides
            # whether this screenshot documents the page the run opened.
            $ours = [Wsa]::PointIsOn($hWnd, $px, $py)
            if ($try -eq 0) {
                if ($ours) {
                    Write-Verbose '  under the cursor: ours -- clicking the screen'
                } else {
                    Write-Verbose '  under the cursor: another window -- posting to ours'
                }
            }
            if ($ours) {
                [Wsa]::ClickAt($px, $py)
            } else {
                [Wsa]::ClickClient($hWnd, [int]($actWhere.x * $scale), [int]($actWhere.y * $scale))
            }
            Start-Sleep -Milliseconds 500
            if ($step -like 'go-*') {
                $applied = LogGained $LogDir $logSince 'tab selected'
            } else {
                # No detector for an in-page tab switch -- the app does not log
                # it -- so it takes the repeats it is entitled to rather than
                # stopping after the first click. Three clicks on a control that
                # sets a value end where one does; only the *last* attempt counts
                # as done, which is what makes it actually retry.
                $applied = ($try -eq ($attempts - 1))
            }
            if (-not $applied) {
                Write-Verbose ("'{0}' attempt {1} not confirmed yet" -f $step, ($try + 1))
            }
        }
        if (-not $applied) {
            throw ("-Act '{0}' never reached a page: the app logged no 'tab selected' " +
                   "after {1} clicks, so a capture now would photograph whichever page " +
                   "was already showing. See {2}") -f $step, $attempts, $LogDir
        }

        # The side bar's width animates over 200ms and the theme switch over
        # 150ms; waiting past both is cheaper than either animation being
        # halfway through the shutter.
        Start-Sleep -Milliseconds 700
    }

    if ($steps.Count -gt 0) {
        # Off the window before the shutter.
        #
        # The last click leaves the cursor sitting on a button, so the picture
        # carries that button's tooltip -- clipped by the window's edge, which
        # reads as a layout bug the app does not have (the reference's tooltip
        # is positioned the same way and would clip identically). Hover states
        # are worth nothing in an evidence shot and a tooltip is worth less, so
        # the cursor goes where it cannot be over anything.
        $last = [Wsa]::Frame($hWnd)
        [Wsa]::MoveAway($last.Left - 120, $last.Top - 120)
        Start-Sleep -Milliseconds 250
    }

    # The size the four captures share: `app.slint`'s 800x600, in the physical
    # pixels this monitor actually has (1200x900 at 150%). If the window opened
    # at anything else the four are not comparable, so that is refused rather
    # than resized -- an app that remembered a size would otherwise silently
    # produce four pictures that cannot be laid side by side.
    $client = [Wsa]::ClientSize($hWnd)
    if ($client.Width -ne $expectedW -or $client.Height -ne $expectedH) {
        throw ('the window''s client area is ' + $client.Width + 'x' + $client.Height +
               ', not the ' + $expectedW + 'x' + $expectedH + ' that app.slint declares at this ' +
               'monitor''s scale. The four captures have to be the same shape to be worth ' +
               'comparing, and resizing the window is not this script''s business.')
    }

    $bitmap = $null
    $best = 0
    $dominant = ''
    $blocked = ''
    foreach ($attempt in 1..8) {
        # Three things a screen capture has to be sure of, in the order they
        # can go wrong: is this our window, is anything over it, and is what
        # comes back the page. All three are asked again on every pass because
        # each is a question about *this instant* -- a rectangle read before the
        # shutter opens is a rectangle about something that has since moved.
        $frame = [Wsa]::Frame($hWnd)
        $fw = $frame.Right - $frame.Left
        $fh = $frame.Bottom - $frame.Top
        if ($fw -ne $expectedW -or $fh -ne $expectedH) {
            throw ('the window is ' + $fw + 'x' + $fh + ', not the ' + $expectedW + 'x' +
                   $expectedH + ' that app.slint declares at this monitor''s scale. The four ' +
                   'captures have to be the same shape to be worth comparing, and resizing the ' +
                   'window is not this script''s business.')
        }

        # Raising is separate from the question because the question is not a
        # way to raise anything: `WindowFromPoint` reports what is in front, and
        # our own attempt to get in front has to happen first.
        [Wsa]::Raise($hWnd)
        $blocked = Get-ForeignPoint $hWnd $frame
        if ($blocked) {
            Start-Sleep -Milliseconds 700
            continue
        }

        [Wsa]::WarmDesktopSurface()
        $cand = New-Object System.Drawing.Bitmap($fw, $fh)
        $g = [System.Drawing.Graphics]::FromImage($cand)
        try { $g.CopyFromScreen($frame.Left, $frame.Top, 0, 0, (New-Object System.Drawing.Size($fw, $fh))) }
        finally { $g.Dispose() }

        $diversity = Get-ContentDiversity $cand
        if ($diversity -gt $best) { $best = $diversity }
        if ($diversity -lt $MinDiversity) {
            $cand.Dispose()
            Start-Sleep -Milliseconds 800
            continue
        }
        $dominant = Get-DominantColour $cand
        if ($dominant.ToUpperInvariant() -ne $ExpectedBackground.ToUpperInvariant()) {
            $cand.Dispose()
            # Once per run, and only once: pausing the filter and resuming it
            # mid-loop would leave the desktop in whichever state the run ended
            # in, which is what the `finally` exists to prevent.
            if ($BypassDisplayFilter -and -not $script:FilterSuspended) {
                Suspend-DisplayFilter
                Write-Output ('the screen was measuring the page as ' + $dominant +
                              '; the display filter is paused and will be restarted after the capture.')
            }
            Start-Sleep -Milliseconds 800
            continue
        }
        $bitmap = $cand
        break
    }

    if (-not $bitmap) {
        if ($blocked) {
            throw ('another window will not stay behind ours: ' + $blocked + '. Photographing it ' +
                   'would produce a correct-looking file of the wrong application. Raise ours, or ' +
                   'turn off that window''s "always on top" if it has one, and re-run.')
        }
        if ($dominant -and $dominant.ToUpperInvariant() -ne $ExpectedBackground.ToUpperInvariant()) {
            $why = if (@($Act -split ',') -contains 'dark-theme') {
                'the dark theme is on and the expected background is still the light one ' +
                '(Theme.content_bg is #1E1E1E dark and #FFFFFF light, and the dominant area is ' +
                'the side bar, #2D2D30). Pass -ExpectedBackground for the theme you are shooting.'
            } else {
                "That is the screen's doing rather than the application's: f.lux composes through " +
                'DWM over the whole desktop on this machine and makes white come back #FFA250. ' +
                'Turn it off for a moment and re-run.'
            }
            throw ('the capture''s dominant colour is ' + $dominant + ' and this run expected ' +
                   $ExpectedBackground + ' -- ' + $why)
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
    Remove-Item Env:RUST_LOG -ErrorAction SilentlyContinue
    # Last, and unconditional: a refusal anywhere above must not leave the
    # reader's screen filter switched off.
    Restore-DisplayFilter
}
