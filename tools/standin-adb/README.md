# SPDX-FileCopyrightText: Copyright (c) 2026
# SPDX-License-Identifier: GPL-3.0-only

# Stand-in adb executables.
#
# Four `.cmd` files, one per tier the home page can render. They exist for the
# same reason the unit tests' fake adb does (Q8): no test and no screenshot may
# need a running WSA, and a page that only ever renders its Ready tier cannot
# be checked for the behaviour spec section 5 is entirely about -- not going
# blank when the subsystem cannot answer.
#
# Point the GUI at one and it is used verbatim in place of `adb`:
#
#     $env:WSADASH_ADB = "$PWD\tools\standin-adb\adb-ready.cmd"
#     & .\target\release\wsadashboard.exe
#
# | file                | `adb devices`      | `sys.boot_completed` | state         | tier    | what the page shows            |
# |---------------------|--------------------|----------------------|---------------|---------|--------------------------------|
# | `adb-ready.cmd`     | `127.0.0.1:58526 device` | `1`             | `Ready`       | Ready   | the app list, no degraded element |
# | `adb-booting.cmd`   | `127.0.0.1:58526 device` | (empty)        | `Booting`     | Partial | the reference's loading overlay  |
# | `adb-offline.cmd`   | `127.0.0.1:58526 offline` | --             | `Offline`     | Partial | skeleton rows + "connection lost" |
# | `adb-stopped.cmd`   | (no rows)          | --                   | `Stopped`     | Down    | `DistroNotFoundView` + "Start WSA" |
#
# The port is 58526 because that is what this machine's WSA manifest declares
# (`adb::port::FALLBACK_PORT` matches it too). `probe()` looks for the endpoint
# by serial, so a stand-in that used any other port would report `Stopped` and
# look like a bug in the page rather than in the stand-in.
#
# Each script is pure ASCII and takes no paths from its own location, so the
# directory can be copied anywhere. They are `.cmd` because
# `std::process::Command` executes one through `cmd.exe /c` on Windows, which
# is what the executor does with whatever `WSADASH_ADB` names.
#
# These are NOT the unit tests' fake adb. That one lives in a temp directory
# and reads its replies from files so a test can decide what adb says per
# call; these are self-contained, because a screenshot is of one fixed state
# and a second file per state is a second thing to keep in step with the first.
