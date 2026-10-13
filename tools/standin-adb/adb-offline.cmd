@echo off
rem SPDX-FileCopyrightText: Copyright (c) 2026
rem SPDX-License-Identifier: GPL-3.0-only
rem
rem Stand-in adb: the endpoint EXISTS but has NO SESSION.
rem
rem     $env:WSADASH_ADB = "...tools\standin-adb\adb-offline.cmd"
rem
rem This is what killing WsaClient.exe leaves behind (spec 1.3): the entry
rem ages out of `adb devices` over the following minutes. It is
rem `SubsystemState::Offline`, which is a Partial tier, so the page shows
rem skeleton rows and the "connection lost" toast -- NOT the not-installed
rem view, because a subsystem with a stale entry is still running and
rem pressing start on it would do nothing useful.

setlocal
if "%~1"=="devices" goto devices
if "%~1"=="connect" goto connect
if "%~1"=="shell" goto shell

echo standin-adb: no rule for %* 1>&2
exit /b 1

:devices
echo List of devices attached
echo 127.0.0.1:58526	offline
exit /b 0

:connect
echo connected to %~2
exit /b 0

:shell
echo standin-adb: no rule for shell %* 1>&2
exit /b 1
