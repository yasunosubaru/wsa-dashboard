@echo off
rem SPDX-FileCopyrightText: Copyright (c) 2026
rem SPDX-License-Identifier: GPL-3.0-only
rem
rem Stand-in adb: the subsystem has NO ENDPOINT AT ALL.
rem
rem     $env:WSADASH_ADB = "...tools\standin-adb\adb-stopped.cmd"
rem
rem `probe()` sees an absent entry, tries `adb connect`, is refused, lists
rem again, and reports `SubsystemState::Stopped` -- provided WSA is actually
rem registered with Appx on this machine, which the probe checks separately.
rem Stopped is a Down tier, so the page renders `DistroNotFoundView` with the
rem "Start WSA" link in its action slot.
rem
rem The refused connect matters: a stand-in that answered it would report a
rem session that does not exist and the page would claim a tier it is not in.

setlocal
if "%~1"=="devices" goto devices
if "%~1"=="connect" goto connect
if "%~1"=="shell" goto shell

echo standin-adb: no rule for %* 1>&2
exit /b 1

:devices
echo List of devices attached
exit /b 0

:connect
echo failed to connect to %~2: Connection refused 1>&2
exit /b 1

:shell
echo standin-adb: no rule for shell %* 1>&2
exit /b 1
