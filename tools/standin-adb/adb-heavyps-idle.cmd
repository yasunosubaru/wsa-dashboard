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
echo package:org.mozilla.firefox
echo package:com.spotify.music
echo package:com.whatsapp
exit /b 0

:ps
echo USER PID PPID VSZ RSS WCHAN ADDR S NAME
echo u0_a101 2001 567 1234567 89012 SyS_epoll+ 0 S system_service_001
echo u0_a102 2002 567 1234567 89012 SyS_epoll+ 0 S system_service_002
echo u0_a103 2003 567 1234567 89012 SyS_epoll+ 0 S system_service_003
echo u0_a104 2004 567 1234567 89012 SyS_epoll+ 0 S system_service_004
echo u0_a105 2005 567 1234567 89012 SyS_epoll+ 0 S system_service_005
echo u0_a106 2006 567 1234567 89012 SyS_epoll+ 0 S system_service_006
echo u0_a107 2007 567 1234567 89012 SyS_epoll+ 0 S system_service_007
echo u0_a108 2008 567 1234567 89012 SyS_epoll+ 0 S system_service_008
echo u0_a109 2009 567 1234567 89012 SyS_epoll+ 0 S system_service_009
echo u0_a110 2010 567 1234567 89012 SyS_epoll+ 0 S system_service_010
echo u0_a111 2011 567 1234567 89012 SyS_epoll+ 0 S system_service_011
echo u0_a112 2012 567 1234567 89012 SyS_epoll+ 0 S system_service_012
echo u0_a113 2013 567 1234567 89012 SyS_epoll+ 0 S system_service_013
echo u0_a114 2014 567 1234567 89012 SyS_epoll+ 0 S system_service_014
echo u0_a115 2015 567 1234567 89012 SyS_epoll+ 0 S system_service_015
echo u0_a116 2016 567 1234567 89012 SyS_epoll+ 0 S system_service_016
echo u0_a117 2017 567 1234567 89012 SyS_epoll+ 0 S system_service_017
echo u0_a118 2018 567 1234567 89012 SyS_epoll+ 0 S system_service_018
echo u0_a119 2019 567 1234567 89012 SyS_epoll+ 0 S system_service_019
echo u0_a120 2020 567 1234567 89012 SyS_epoll+ 0 S system_service_020
echo u0_a121 2021 567 1234567 89012 SyS_epoll+ 0 S system_service_021
echo u0_a122 2022 567 1234567 89012 SyS_epoll+ 0 S system_service_022
echo u0_a123 2023 567 1234567 89012 SyS_epoll+ 0 S system_service_023
echo u0_a124 2024 567 1234567 89012 SyS_epoll+ 0 S system_service_024
echo u0_a125 2025 567 1234567 89012 SyS_epoll+ 0 S system_service_025
echo u0_a126 2026 567 1234567 89012 SyS_epoll+ 0 S system_service_026
echo u0_a127 2027 567 1234567 89012 SyS_epoll+ 0 S system_service_027
echo u0_a128 2028 567 1234567 89012 SyS_epoll+ 0 S system_service_028
echo u0_a129 2029 567 1234567 89012 SyS_epoll+ 0 S system_service_029
echo u0_a130 2030 567 1234567 89012 SyS_epoll+ 0 S system_service_030
echo u0_a131 2031 567 1234567 89012 SyS_epoll+ 0 S system_service_031
echo u0_a132 2032 567 1234567 89012 SyS_epoll+ 0 S system_service_032
echo u0_a133 2033 567 1234567 89012 SyS_epoll+ 0 S system_service_033
echo u0_a134 2034 567 1234567 89012 SyS_epoll+ 0 S system_service_034
echo u0_a135 2035 567 1234567 89012 SyS_epoll+ 0 S system_service_035
echo u0_a136 2036 567 1234567 89012 SyS_epoll+ 0 S system_service_036
echo u0_a137 2037 567 1234567 89012 SyS_epoll+ 0 S system_service_037
echo u0_a138 2038 567 1234567 89012 SyS_epoll+ 0 S system_service_038
echo u0_a139 2039 567 1234567 89012 SyS_epoll+ 0 S system_service_039
echo u0_a140 2040 567 1234567 89012 SyS_epoll+ 0 S system_service_040
echo u0_a141 2041 567 1234567 89012 SyS_epoll+ 0 S system_service_041
echo u0_a142 2042 567 1234567 89012 SyS_epoll+ 0 S system_service_042
echo u0_a143 2043 567 1234567 89012 SyS_epoll+ 0 S system_service_043
echo u0_a144 2044 567 1234567 89012 SyS_epoll+ 0 S system_service_044
echo u0_a145 2045 567 1234567 89012 SyS_epoll+ 0 S system_service_045
echo u0_a146 2046 567 1234567 89012 SyS_epoll+ 0 S system_service_046
echo u0_a147 2047 567 1234567 89012 SyS_epoll+ 0 S system_service_047
echo u0_a148 2048 567 1234567 89012 SyS_epoll+ 0 S system_service_048
echo u0_a149 2049 567 1234567 89012 SyS_epoll+ 0 S system_service_049
echo u0_a150 2050 567 1234567 89012 SyS_epoll+ 0 S system_service_050
echo u0_a151 2051 567 1234567 89012 SyS_epoll+ 0 S system_service_051
echo u0_a152 2052 567 1234567 89012 SyS_epoll+ 0 S system_service_052
echo u0_a153 2053 567 1234567 89012 SyS_epoll+ 0 S system_service_053
echo u0_a154 2054 567 1234567 89012 SyS_epoll+ 0 S system_service_054
echo u0_a155 2055 567 1234567 89012 SyS_epoll+ 0 S system_service_055
echo u0_a156 2056 567 1234567 89012 SyS_epoll+ 0 S system_service_056
echo u0_a157 2057 567 1234567 89012 SyS_epoll+ 0 S system_service_057
echo u0_a158 2058 567 1234567 89012 SyS_epoll+ 0 S system_service_058
echo u0_a159 2059 567 1234567 89012 SyS_epoll+ 0 S system_service_059
echo u0_a160 2060 567 1234567 89012 SyS_epoll+ 0 S system_service_060
echo u0_a161 2061 567 1234567 89012 SyS_epoll+ 0 S system_service_061
echo u0_a162 2062 567 1234567 89012 SyS_epoll+ 0 S system_service_062
echo u0_a163 2063 567 1234567 89012 SyS_epoll+ 0 S system_service_063
echo u0_a164 2064 567 1234567 89012 SyS_epoll+ 0 S system_service_064
echo u0_a165 2065 567 1234567 89012 SyS_epoll+ 0 S system_service_065
echo u0_a166 2066 567 1234567 89012 SyS_epoll+ 0 S system_service_066
echo u0_a167 2067 567 1234567 89012 SyS_epoll+ 0 S system_service_067
echo u0_a168 2068 567 1234567 89012 SyS_epoll+ 0 S system_service_068
echo u0_a169 2069 567 1234567 89012 SyS_epoll+ 0 S system_service_069
echo u0_a170 2070 567 1234567 89012 SyS_epoll+ 0 S system_service_070
echo u0_a171 2071 567 1234567 89012 SyS_epoll+ 0 S system_service_071
echo u0_a172 2072 567 1234567 89012 SyS_epoll+ 0 S system_service_072
echo u0_a173 2073 567 1234567 89012 SyS_epoll+ 0 S system_service_073
echo u0_a174 2074 567 1234567 89012 SyS_epoll+ 0 S system_service_074
echo u0_a175 2075 567 1234567 89012 SyS_epoll+ 0 S system_service_075
echo u0_a176 2076 567 1234567 89012 SyS_epoll+ 0 S system_service_076
echo u0_a177 2077 567 1234567 89012 SyS_epoll+ 0 S system_service_077
echo u0_a178 2078 567 1234567 89012 SyS_epoll+ 0 S system_service_078
echo u0_a179 2079 567 1234567 89012 SyS_epoll+ 0 S system_service_079
echo u0_a180 2080 567 1234567 89012 SyS_epoll+ 0 S system_service_080
echo u0_a181 2081 567 1234567 89012 SyS_epoll+ 0 S system_service_081
echo u0_a182 2082 567 1234567 89012 SyS_epoll+ 0 S system_service_082
echo u0_a183 2083 567 1234567 89012 SyS_epoll+ 0 S system_service_083
echo u0_a184 2084 567 1234567 89012 SyS_epoll+ 0 S system_service_084
echo u0_a185 2085 567 1234567 89012 SyS_epoll+ 0 S system_service_085
echo u0_a186 2086 567 1234567 89012 SyS_epoll+ 0 S system_service_086
echo u0_a187 2087 567 1234567 89012 SyS_epoll+ 0 S system_service_087
echo u0_a188 2088 567 1234567 89012 SyS_epoll+ 0 S system_service_088
echo u0_a189 2089 567 1234567 89012 SyS_epoll+ 0 S system_service_089
echo u0_a190 2090 567 1234567 89012 SyS_epoll+ 0 S system_service_090
echo u0_a191 2091 567 1234567 89012 SyS_epoll+ 0 S system_service_091
echo u0_a192 2092 567 1234567 89012 SyS_epoll+ 0 S system_service_092
echo u0_a193 2093 567 1234567 89012 SyS_epoll+ 0 S system_service_093
echo u0_a194 2094 567 1234567 89012 SyS_epoll+ 0 S system_service_094
echo u0_a195 2095 567 1234567 89012 SyS_epoll+ 0 S system_service_095
echo u0_a196 2096 567 1234567 89012 SyS_epoll+ 0 S system_service_096
echo u0_a197 2097 567 1234567 89012 SyS_epoll+ 0 S system_service_097
echo u0_a198 2098 567 1234567 89012 SyS_epoll+ 0 S system_service_098
echo u0_a199 2099 567 1234567 89012 SyS_epoll+ 0 S system_service_099
echo u0_a200 2100 567 1234567 89012 SyS_epoll+ 0 S system_service_100
echo u0_a201 2101 567 1234567 89012 SyS_epoll+ 0 S system_service_101
echo u0_a202 2102 567 1234567 89012 SyS_epoll+ 0 S system_service_102
echo u0_a203 2103 567 1234567 89012 SyS_epoll+ 0 S system_service_103
echo u0_a204 2104 567 1234567 89012 SyS_epoll+ 0 S system_service_104
echo u0_a205 2105 567 1234567 89012 SyS_epoll+ 0 S system_service_105
echo u0_a206 2106 567 1234567 89012 SyS_epoll+ 0 S system_service_106
echo u0_a207 2107 567 1234567 89012 SyS_epoll+ 0 S system_service_107
echo u0_a208 2108 567 1234567 89012 SyS_epoll+ 0 S system_service_108
echo u0_a209 2109 567 1234567 89012 SyS_epoll+ 0 S system_service_109
echo u0_a210 2110 567 1234567 89012 SyS_epoll+ 0 S system_service_110
echo u0_a211 2111 567 1234567 89012 SyS_epoll+ 0 S system_service_111
echo u0_a212 2112 567 1234567 89012 SyS_epoll+ 0 S system_service_112
echo u0_a213 2113 567 1234567 89012 SyS_epoll+ 0 S system_service_113
echo u0_a214 2114 567 1234567 89012 SyS_epoll+ 0 S system_service_114
echo u0_a215 2115 567 1234567 89012 SyS_epoll+ 0 S system_service_115
echo u0_a216 2116 567 1234567 89012 SyS_epoll+ 0 S system_service_116
echo u0_a217 2117 567 1234567 89012 SyS_epoll+ 0 S system_service_117
echo u0_a218 2118 567 1234567 89012 SyS_epoll+ 0 S system_service_118
echo u0_a219 2119 567 1234567 89012 SyS_epoll+ 0 S system_service_119
echo u0_a220 2120 567 1234567 89012 SyS_epoll+ 0 S system_service_120
echo u0_a221 2121 567 1234567 89012 SyS_epoll+ 0 S system_service_121
echo u0_a222 2122 567 1234567 89012 SyS_epoll+ 0 S system_service_122
echo u0_a223 2123 567 1234567 89012 SyS_epoll+ 0 S system_service_123
echo u0_a224 2124 567 1234567 89012 SyS_epoll+ 0 S system_service_124
echo u0_a225 2125 567 1234567 89012 SyS_epoll+ 0 S system_service_125
echo u0_a226 2126 567 1234567 89012 SyS_epoll+ 0 S system_service_126
echo u0_a227 2127 567 1234567 89012 SyS_epoll+ 0 S system_service_127
echo u0_a228 2128 567 1234567 89012 SyS_epoll+ 0 S system_service_128
echo u0_a229 2129 567 1234567 89012 SyS_epoll+ 0 S system_service_129
echo u0_a230 2130 567 1234567 89012 SyS_epoll+ 0 S system_service_130
echo u0_a231 2131 567 1234567 89012 SyS_epoll+ 0 S system_service_131
echo u0_a232 2132 567 1234567 89012 SyS_epoll+ 0 S system_service_132
echo u0_a233 2133 567 1234567 89012 SyS_epoll+ 0 S system_service_133
echo u0_a234 2134 567 1234567 89012 SyS_epoll+ 0 S system_service_134
echo u0_a235 2135 567 1234567 89012 SyS_epoll+ 0 S system_service_135
echo u0_a236 2136 567 1234567 89012 SyS_epoll+ 0 S system_service_136
echo u0_a237 2137 567 1234567 89012 SyS_epoll+ 0 S system_service_137
echo u0_a238 2138 567 1234567 89012 SyS_epoll+ 0 S system_service_138
echo u0_a239 2139 567 1234567 89012 SyS_epoll+ 0 S system_service_139
echo u0_a240 2140 567 1234567 89012 SyS_epoll+ 0 S system_service_140
echo u0_a241 2141 567 1234567 89012 SyS_epoll+ 0 S system_service_141
echo u0_a242 2142 567 1234567 89012 SyS_epoll+ 0 S system_service_142
echo u0_a243 2143 567 1234567 89012 SyS_epoll+ 0 S system_service_143
echo u0_a244 2144 567 1234567 89012 SyS_epoll+ 0 S system_service_144
echo u0_a245 2145 567 1234567 89012 SyS_epoll+ 0 S system_service_145
echo u0_a246 2146 567 1234567 89012 SyS_epoll+ 0 S system_service_146
echo u0_a247 2147 567 1234567 89012 SyS_epoll+ 0 S system_service_147
echo u0_a248 2148 567 1234567 89012 SyS_epoll+ 0 S system_service_148
echo u0_a249 2149 567 1234567 89012 SyS_epoll+ 0 S system_service_149
echo u0_a250 2150 567 1234567 89012 SyS_epoll+ 0 S system_service_150
echo u0_a251 2151 567 1234567 89012 SyS_epoll+ 0 S system_service_151
echo u0_a252 2152 567 1234567 89012 SyS_epoll+ 0 S system_service_152
echo u0_a253 2153 567 1234567 89012 SyS_epoll+ 0 S system_service_153
echo u0_a254 2154 567 1234567 89012 SyS_epoll+ 0 S system_service_154
echo u0_a255 2155 567 1234567 89012 SyS_epoll+ 0 S system_service_155
echo u0_a256 2156 567 1234567 89012 SyS_epoll+ 0 S system_service_156
echo u0_a257 2157 567 1234567 89012 SyS_epoll+ 0 S system_service_157
echo u0_a258 2158 567 1234567 89012 SyS_epoll+ 0 S system_service_158
echo u0_a259 2159 567 1234567 89012 SyS_epoll+ 0 S system_service_159
echo u0_a260 2160 567 1234567 89012 SyS_epoll+ 0 S system_service_160
echo u0_a261 2161 567 1234567 89012 SyS_epoll+ 0 S system_service_161
echo u0_a262 2162 567 1234567 89012 SyS_epoll+ 0 S system_service_162
echo u0_a263 2163 567 1234567 89012 SyS_epoll+ 0 S system_service_163
echo u0_a264 2164 567 1234567 89012 SyS_epoll+ 0 S system_service_164
echo u0_a265 2165 567 1234567 89012 SyS_epoll+ 0 S system_service_165
echo u0_a266 2166 567 1234567 89012 SyS_epoll+ 0 S system_service_166
echo u0_a267 2167 567 1234567 89012 SyS_epoll+ 0 S system_service_167
echo u0_a268 2168 567 1234567 89012 SyS_epoll+ 0 S system_service_168
echo u0_a269 2169 567 1234567 89012 SyS_epoll+ 0 S system_service_169
echo u0_a270 2170 567 1234567 89012 SyS_epoll+ 0 S system_service_170
echo u0_a271 2171 567 1234567 89012 SyS_epoll+ 0 S system_service_171
echo u0_a272 2172 567 1234567 89012 SyS_epoll+ 0 S system_service_172
echo u0_a273 2173 567 1234567 89012 SyS_epoll+ 0 S system_service_173
echo u0_a274 2174 567 1234567 89012 SyS_epoll+ 0 S system_service_174
echo u0_a275 2175 567 1234567 89012 SyS_epoll+ 0 S system_service_175
echo u0_a276 2176 567 1234567 89012 SyS_epoll+ 0 S system_service_176
echo u0_a277 2177 567 1234567 89012 SyS_epoll+ 0 S system_service_177
echo u0_a278 2178 567 1234567 89012 SyS_epoll+ 0 S system_service_178
echo u0_a279 2179 567 1234567 89012 SyS_epoll+ 0 S system_service_179
echo u0_a280 2180 567 1234567 89012 SyS_epoll+ 0 S system_service_180
echo u0_a281 2181 567 1234567 89012 SyS_epoll+ 0 S system_service_181
echo u0_a282 2182 567 1234567 89012 SyS_epoll+ 0 S system_service_182
echo u0_a283 2183 567 1234567 89012 SyS_epoll+ 0 S system_service_183
echo u0_a284 2184 567 1234567 89012 SyS_epoll+ 0 S system_service_184
echo u0_a285 2185 567 1234567 89012 SyS_epoll+ 0 S system_service_185
echo u0_a286 2186 567 1234567 89012 SyS_epoll+ 0 S system_service_186
echo u0_a287 2187 567 1234567 89012 SyS_epoll+ 0 S system_service_187
echo u0_a288 2188 567 1234567 89012 SyS_epoll+ 0 S system_service_188
echo u0_a289 2189 567 1234567 89012 SyS_epoll+ 0 S system_service_189
echo u0_a290 2190 567 1234567 89012 SyS_epoll+ 0 S system_service_190
echo u0_a291 2191 567 1234567 89012 SyS_epoll+ 0 S system_service_191
echo u0_a292 2192 567 1234567 89012 SyS_epoll+ 0 S system_service_192
echo u0_a293 2193 567 1234567 89012 SyS_epoll+ 0 S system_service_193
echo u0_a294 2194 567 1234567 89012 SyS_epoll+ 0 S system_service_194
echo u0_a295 2195 567 1234567 89012 SyS_epoll+ 0 S system_service_195
echo u0_a296 2196 567 1234567 89012 SyS_epoll+ 0 S system_service_196
echo u0_a297 2197 567 1234567 89012 SyS_epoll+ 0 S system_service_197
echo u0_a298 2198 567 1234567 89012 SyS_epoll+ 0 S system_service_198
echo u0_a299 2199 567 1234567 89012 SyS_epoll+ 0 S system_service_199
echo u0_a300 2200 567 1234567 89012 SyS_epoll+ 0 S system_service_200
exit /b 0
echo USER PID PPID VSZ RSS WCHAN ADDR S NAME
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
