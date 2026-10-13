@echo off
rem SPDX-FileCopyrightText: Copyright (c) 2026
rem SPDX-License-Identifier: GPL-3.0-only
rem
rem Stand-in adb: the subsystem is UP AND BOOT COMPLETE.
rem
rem Point the GUI at it with
rem     $env:WSADASH_ADB = "<repo>\tools\standin-adb\adb-ready.cmd"
rem and the home page renders its Ready tier -- the app list, exactly as the
rem reference renders a distro list, with no degraded element anywhere.
rem
rem Pure ASCII, and no path is baked in: the port is matched against the
rem `LocalPort` this machine's manifest actually declares (58526), which is
rem what `probe()` looks for. A tab separates the serial from the state,
rem because that is the separator `adb devices` uses.

setlocal
if "%~1"=="devices" goto devices
if "%~1"=="connect" goto connect
if "%~1"=="shell" goto shell

echo standin-adb: no rule for %* 1>&2
exit /b 1

:devices
echo List of devices attached
echo 127.0.0.1:58526	device
exit /b 0

:connect
echo connected to %~2
exit /b 0

:shell
if "%~2"=="getprop" goto getprop
if "%~2"=="ps" goto ps
if "%~2"=="pm" goto pm
if "%~2"=="dumpsys" goto dumpsys
if "%~2"=="am" goto am
echo standin-adb: no rule for shell %* 1>&2
exit /b 1

:getprop
if "%~3"=="sys.boot_completed" (
  echo 1
  exit /b 0
)
echo %~3=
exit /b 0

:pm
echo package:com.topjohnwu.magisk
echo package:org.mozilla.firefox
echo package:com.spotify.music
echo package:com.whatsapp
exit /b 0

:ps
echo USER PID PPID VSZ RSS WCHAN ADDR S NAME
echo u0_a234 1234 567 1234567 89012 SyS_epoll+ 0 S com.topjohnwu.magisk
echo u0_a251 1251 567 2345678 90123 SyS_epoll+ 0 S org.mozilla.firefox
exit /b 0

:dumpsys
echo Packages:
echo   Package [%~4] (deadbeef):
echo     userId=10234
echo     versionName=28.1
echo     minSdk=29
echo     targetSdk=34
echo     firstInstallTime=2026-01-01 10:00:00
exit /b 0

:am
exit /b 0
