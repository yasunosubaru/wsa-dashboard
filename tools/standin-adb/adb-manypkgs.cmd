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
if "%~3"=="" goto allprops
if "%~3"=="sys.boot_completed" (
  echo 1
  exit /b 0
)
echo %~3=
exit /b 0

:pm
echo package:com.example.app001
echo package:com.example.app002
echo package:com.example.app003
echo package:com.example.app004
echo package:com.example.app005
echo package:com.example.app006
echo package:com.example.app007
echo package:com.example.app008
echo package:com.example.app009
echo package:com.example.app010
echo package:com.example.app011
echo package:com.example.app012
echo package:com.example.app013
echo package:com.example.app014
echo package:com.example.app015
echo package:com.example.app016
echo package:com.example.app017
echo package:com.example.app018
echo package:com.example.app019
echo package:com.example.app020
echo package:com.example.app021
echo package:com.example.app022
echo package:com.example.app023
echo package:com.example.app024
echo package:com.example.app025
echo package:com.example.app026
echo package:com.example.app027
echo package:com.example.app028
echo package:com.example.app029
echo package:com.example.app030
echo package:com.example.app031
echo package:com.example.app032
echo package:com.example.app033
echo package:com.example.app034
echo package:com.example.app035
echo package:com.example.app036
echo package:com.example.app037
echo package:com.example.app038
echo package:com.example.app039
echo package:com.example.app040
echo package:com.example.app041
echo package:com.example.app042
echo package:com.example.app043
echo package:com.example.app044
echo package:com.example.app045
echo package:com.example.app046
echo package:com.example.app047
echo package:com.example.app048
echo package:com.example.app049
echo package:com.example.app050
echo package:com.example.app051
echo package:com.example.app052
echo package:com.example.app053
echo package:com.example.app054
echo package:com.example.app055
echo package:com.example.app056
echo package:com.example.app057
echo package:com.example.app058
echo package:com.example.app059
echo package:com.example.app060
echo package:com.example.app061
echo package:com.example.app062
echo package:com.example.app063
echo package:com.example.app064
echo package:com.example.app065
echo package:com.example.app066
echo package:com.example.app067
echo package:com.example.app068
echo package:com.example.app069
echo package:com.example.app070
echo package:com.example.app071
echo package:com.example.app072
echo package:com.example.app073
echo package:com.example.app074
echo package:com.example.app075
echo package:com.example.app076
echo package:com.example.app077
echo package:com.example.app078
echo package:com.example.app079
echo package:com.example.app080
echo package:com.example.app081
echo package:com.example.app082
echo package:com.example.app083
echo package:com.example.app084
echo package:com.example.app085
echo package:com.example.app086
echo package:com.example.app087
echo package:com.example.app088
echo package:com.example.app089
echo package:com.example.app090
echo package:com.example.app091
echo package:com.example.app092
echo package:com.example.app093
echo package:com.example.app094
echo package:com.example.app095
echo package:com.example.app096
echo package:com.example.app097
echo package:com.example.app098
echo package:com.example.app099
echo package:com.example.app100
echo package:com.example.app101
echo package:com.example.app102
echo package:com.example.app103
echo package:com.example.app104
echo package:com.example.app105
echo package:com.example.app106
echo package:com.example.app107
echo package:com.example.app108
echo package:com.example.app109
echo package:com.example.app110
echo package:com.example.app111
echo package:com.example.app112
echo package:com.example.app113
echo package:com.example.app114
echo package:com.example.app115
echo package:com.example.app116
echo package:com.example.app117
echo package:com.example.app118
echo package:com.example.app119
echo package:com.example.app120
echo package:com.example.app121
echo package:com.example.app122
echo package:com.example.app123
echo package:com.example.app124
echo package:com.example.app125
echo package:com.example.app126
echo package:com.example.app127
echo package:com.example.app128
echo package:com.example.app129
echo package:com.example.app130
echo package:com.example.app131
echo package:com.example.app132
echo package:com.example.app133
echo package:com.example.app134
echo package:com.example.app135
echo package:com.example.app136
echo package:com.example.app137
echo package:com.example.app138
echo package:com.example.app139
echo package:com.example.app140
echo package:com.example.app141
echo package:com.example.app142
echo package:com.example.app143
echo package:com.example.app144
echo package:com.example.app145
echo package:com.example.app146
echo package:com.example.app147
echo package:com.example.app148
echo package:com.example.app149
echo package:com.example.app150
echo package:com.example.app151
echo package:com.example.app152
echo package:com.example.app153
echo package:com.example.app154
echo package:com.example.app155
echo package:com.example.app156
echo package:com.example.app157
echo package:com.example.app158
echo package:com.example.app159
echo package:com.example.app160
echo package:com.example.app161
echo package:com.example.app162
echo package:com.example.app163
echo package:com.example.app164
echo package:com.example.app165
echo package:com.example.app166
echo package:com.example.app167
echo package:com.example.app168
echo package:com.example.app169
echo package:com.example.app170
echo package:com.example.app171
echo package:com.example.app172
echo package:com.example.app173
echo package:com.example.app174
echo package:com.example.app175
echo package:com.example.app176
echo package:com.example.app177
echo package:com.example.app178
echo package:com.example.app179
echo package:com.example.app180
echo package:com.example.app181
echo package:com.example.app182
echo package:com.example.app183
echo package:com.example.app184
echo package:com.example.app185
echo package:com.example.app186
echo package:com.example.app187
echo package:com.example.app188
echo package:com.example.app189
echo package:com.example.app190
echo package:com.example.app191
echo package:com.example.app192
echo package:com.example.app193
echo package:com.example.app194
echo package:com.example.app195
echo package:com.example.app196
echo package:com.example.app197
echo package:com.example.app198
echo package:com.example.app199
echo package:com.example.app200
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

:allprops
echo [ro.product.manufacturer]: [Microsoft Corporation]
echo [ro.product.model]: [Subsystem for Android(TM)]
echo [ro.build.version.release]: [13]
echo [ro.build.version.sdk]: [33]
echo [ro.hardware]: [windows_x86_64]
echo [ro.build.display.id]: [TQ3A.230901.001]
echo [ro.build.fingerprint]: [Windows/windows_x86_64/windows_x86_64:13/TQ3A.230901.001/user/release-keys]
exit /b 0
