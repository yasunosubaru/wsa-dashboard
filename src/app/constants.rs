// SPDX-FileCopyrightText: Copyright (c) 2026
// SPDX-License-Identifier: GPL-3.0-only

// Application constants definition.
//
// Symbol names are KEPT IDENTICAL to the reference (wsl-dashboard 0.12.0,
// docs/reference/src/app/constants.rs) so that later phases can be diffed
// line-by-line against the reference. Only VALUES change. See docs/MAPPING.md.
//
// build.rs includes this file via `#[path = "src/app/constants.rs"]`, so every
// symbol here must exist even when the GUI has not been written yet.

#[allow(dead_code)]
pub const APP_NAME: &str = "WSA Dashboard";
#[allow(dead_code)]
pub const APP_ID: &str = "wsadashboard";
#[allow(dead_code)]
pub const COMPANY_NAME: &str = APP_NAME;
#[allow(dead_code)]
pub const LEGAL_COPYRIGHT: &str = "2026 WSA Dashboard. All rights reserved.";

// No remote repository yet (spec Q2: git init locally, no remote until told).
// These stay empty on purpose; the About page renders "not configured" (Q15).
#[allow(dead_code)]
pub const PROJECT_REPOSITORY: &str = "";
#[allow(dead_code)]
pub const PROJECT_WEBSITE: &str = "";
#[allow(dead_code)]
pub const PROJECT_DOCS: &str = "";

#[allow(dead_code)]
pub const GITHUB_ISSUES: &str = "/issues";
#[allow(dead_code)]
pub const GITHUB_RELEASES: &str = "/releases";
#[allow(dead_code)]
pub const DOWNLOAD_URI: &str = "/download/";
#[allow(dead_code)]
pub const DONATE_URI: &str = "/donate/";
#[allow(dead_code)]
pub const PRIVACY_URI: &str = "/privacy/";
#[allow(dead_code)]
pub const TERMS_URI: &str = "/terms/";
#[allow(dead_code)]
pub const GITHUB_DOMAIN: &str = "https://github.com";

// MUST stay empty. The reference hard-codes https://api1.wslui.com and calls
// ten endpoints on it (docs/MAPPING.md section 5); this project never calls
// that host. Update checking goes through GitHub releases only.
#[allow(dead_code)]
pub const API1_URL: &str = "";

// Identifier kept verbatim per spec Q12; value blanked because the reference
// value points at microsoft/WSL releases, which has no WSA counterpart here.
// The "WSA not installed" view supplies its own link when it lands (spec Q28).
#[allow(dead_code)]
pub const WSL_GITHUB_RELEASES: &str = "";

#[allow(dead_code)]
pub const VSCODE_MARKETPLACE_URL: &str = "https://marketplace.visualstudio.com";

// Compatibility of Chinese and Japanese character display on Western language
// operating systems. Domain-neutral -- copied verbatim from the reference.
#[allow(dead_code)]
pub const FONT_ZH: &str = "Microsoft YaHei UI";
#[allow(dead_code)]
pub const FONT_EN_FALLBACK: &str = "Segoe UI, Microsoft YaHei UI";

// Check if a language code represents Chinese
#[allow(dead_code)]
pub fn is_chinese_lang(lang: &str) -> bool {
    lang.to_lowercase().starts_with("zh")
}

// Check if the Windows system locale supports CJK natively.
//
// When the system locale is Chinese, Windows font fallback (DirectWrite)
// automatically renders CJK characters using the system's built-in CJK fonts,
// so we do not need to explicitly specify CJK fonts, saving ~16MB memory by
// avoiding the CJK glyph atlas allocation in Skia.
#[allow(dead_code)]
pub fn system_has_cjk_support(system_lang: &str) -> bool {
    system_lang.to_lowercase().starts_with("zh")
}

// Get the appropriate font for the given app language and Windows system locale.
//
// On Chinese Windows, DirectWrite fallback handles CJK automatically -> use
// "Segoe UI" and skip the large CJK glyph atlas in Skia.
// On non-Chinese Windows, CJK fonts must be specified explicitly.
#[allow(dead_code)]
pub fn get_font_for(app_lang: &str, system_lang: &str) -> &'static str {
    if is_chinese_lang(app_lang) {
        if system_has_cjk_support(system_lang) {
            "Segoe UI"
        } else {
            FONT_ZH
        }
    } else {
        if system_has_cjk_support(system_lang) {
            "Segoe UI"
        } else {
            FONT_EN_FALLBACK
        }
    }
}
