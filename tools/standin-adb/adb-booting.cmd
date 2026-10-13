@echo off
rem SPDX-FileCopyrightText: Copyright (c) 2026
rem SPDX-License-Identifier: GPL-3.0-only
rem
rem Stand-in adb: the subsystem is UP but NOT YET BOOT COMPLETE.
rem
rem     $env:WSADASH_ADB = "...tools\standin-adb\adb-booting.cmd"
rem
rem `adb devices` reports the endpoint, `getprop sys.boot_completed` reads
rem back empty, and the two together are `SubsystemState::Booting` -- the one
rem degraded state whose answer is "wait", so it renders the reference's
rem loading overlay rather than a skeleton.

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
echo standin-adb: no rule for shell %* 1>&2
exit /b 1

:getprop
rem An empty line, not a missing one: the platform prints nothing at all for
rem a key that has not been set yet, and `boot_completed` decides on `1`.
echo.
exit /b 0
