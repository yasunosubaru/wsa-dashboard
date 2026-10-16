// SPDX-FileCopyrightText: Copyright (c) 2026
// SPDX-License-Identifier: GPL-3.0-only

//! Bringing a `settings.toml` written by an older build up to this one.
//!
//! The counterpart of the reference's `src/config/migration.rs`, and the same
//! shape: a stamped `setting-version` in the file, and a chain of steps each
//! guarded by `if old_version < N`. The reference has ten steps because it has
//! shipped ten times; ours has one, and what it is worth writing down is *why*
//! it has one rather than none -- and what a second one will look like.
//!
//! # Why a version marker at all, when every field already has a default
//!
//! `#[serde(default)]` answers "the file does not mention this field". It does
//! **not** answer "the file mentions this field, and the field stopped meaning
//! what it meant". That second case is the one that corrupts: the key parses,
//! the value is a perfectly good `bool`, and the only thing wrong with it is
//! that it was written about a different question. A default cannot fix it,
//! because there is nothing missing to default.
//!
//! This program has already had one of those. In P6, `sparse-vhd` and
//! `mount-auto-probe` kept their key names and the reference's defaults while
//! their *meaning* changed (see MAPPING 15.4): they were WSL settings in the
//! reference and they are storage-page switches here. A file old enough to hold
//! the old meaning cannot be read by the new code without a decision, and the
//! decision belongs in a chain with a version guard rather than in a comment.
//!
//! # What "old enough" means, and what happens if it is wrong
//!
//! [`SETTINGS_VERSION`] is what this build writes; a file with no version at all
//! is version 0 -- that is the reference's convention too (`#[serde(default)]`
//! on the field), and it is also what every file written before this build
//! existed looks like, including one on a machine that has run it for weeks.
//!
//! Two rules the tests pin, both about not making things worse:
//!
//!   * **A newer file is left alone.** A version above this build's means a
//!     build from the future wrote it. Its keys are not ours to reinterpret, and
//!     the reader is more likely to have run the newer build than to be running
//!     this one by mistake -- so it is read as it is and [reported], not
//!     rewritten.
//!   * **The migration never writes.** [`crate::config::load_from`] has one
//!     promise: reading the settings cannot fail and cannot surprise. A read
//!     that rewrote the file would break the first when the disk is read-only
//!     and the second always. The version is stamped when the file is next saved
//!     by the thing that actually changes a setting, which is the reference's
//!     arrangement as well.
//!
//! [reported]: crate::config::load_from

use tracing::{debug, info, warn};

use super::models::Config;

/// The schema this build writes and understands.
///
/// `1` is the first version this program stamped. A file with no
/// `setting-version` reads as `0`, and the one step below is what turns a `0`
/// into a `1`.
pub const SETTINGS_VERSION: u8 = 1;

/// Bring `config` up to [`SETTINGS_VERSION`]. Returns `true` when it changed.
///
/// The return value is not used to write anything -- see this module's docs --
/// but it is what a test can assert on, and it is the difference between "the
/// chain ran" and "the chain had something to do".
pub fn migrate_config(config: &mut Config) -> bool {
    let from = config.application.setting_version;
    if from >= SETTINGS_VERSION {
        if from > SETTINGS_VERSION {
            // Read, not rewritten. See this module's docs.
            warn!(
                "the settings were written by a newer build (v{from} > v{SETTINGS_VERSION}); \
                 reading them as they are"
            );
        }
        return false;
    }

    info!("settings are v{from}; upgrading to v{SETTINGS_VERSION}");
    let mut changed = false;

    // -- v0 -> v1 ------------------------------------------------------------
    //
    // A file with no version is a file that predates this stamp. Two of its keys
    // can no longer be trusted, and they are the reason this step exists: in P6
    // `sparse-vhd` and `mount-auto-probe` kept the reference's names and defaults
    // while the questions they answer changed (MAPPING 15.4). A `v0` file's value
    // for either was written about the WSL question, so it is replaced with this
    // build's default rather than believed.
    //
    // On the files that actually exist it is a no-op -- the shipped default for
    // both was already `false` before the meaning changed -- and that is worth
    // saying rather than hiding: a step that always does nothing and a step that
    // happens to do nothing on today's input look identical in a diff, and only
    // one of them is worth keeping.
    if from < 1 {
        let reset = [
            ("sparse-vhd", &mut config.user.sparse_vhd, false),
            ("mount-auto-probe", &mut config.user.mount_auto_probe, false),
        ];
        for (key, field, default) in reset {
            if *field != default {
                debug!(key, "reset to this build's default; the old value meant something else");
                *field = default;
                changed = true;
            }
        }
    }

    config.application.setting_version = SETTINGS_VERSION;
    if !changed {
        // A chain that ran and moved nothing is still a chain that ran. Said
        // out loud so that a future step which silently stops firing has a
        // baseline to be missing from.
        debug!("nothing needed bringing forward; the version stamp is the change");
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(version: u8) -> Config {
        let mut config = Config::default();
        config.application.setting_version = version;
        config
    }

    #[test]
    fn a_file_with_no_version_is_version_zero_and_gets_stamped() {
        let mut config = Config::default();
        assert_eq!(config.application.setting_version, 0, "serde's default");
        assert!(migrate_config(&mut config), "stamping is a change");
        assert_eq!(config.application.setting_version, SETTINGS_VERSION);
    }

    /// The step's whole job: a value written about the old question is not a
    /// value about the new one.
    #[test]
    fn the_two_keys_that_changed_meaning_are_reset_not_believed() {
        let mut config = at(0);
        config.user.sparse_vhd = true;
        config.user.mount_auto_probe = true;

        assert!(migrate_config(&mut config));
        assert!(!config.user.sparse_vhd);
        assert!(!config.user.mount_auto_probe);
    }

    /// A file already at this version is not touched, and the migration says so
    /// by answering `false`. Without that, "the chain ran" and "the chain had
    /// work to do" would be the same observation.
    #[test]
    fn a_file_at_this_version_is_left_exactly_as_it_is() {
        let mut config = at(SETTINGS_VERSION);
        config.user.sparse_vhd = true; // deliberately not the default any more
        let before = config.clone();

        assert!(!migrate_config(&mut config));
        assert_eq!(config, before, "the reader's value is the reader's");
    }

    /// A file from a build that does not exist yet. Its keys are not ours to
    /// reinterpret, and rewriting it would lose whatever that build put there.
    #[test]
    fn a_file_from_a_newer_build_is_read_and_not_rewritten() {
        let mut config = at(SETTINGS_VERSION + 3);
        config.user.sparse_vhd = true;
        let before = config.clone();

        assert!(!migrate_config(&mut config));
        assert_eq!(config, before);
        assert_eq!(
            config.application.setting_version,
            SETTINGS_VERSION + 3,
            "the newer version stamp must survive"
        );
    }

    /// Every step is guarded by `if from < N`, so a version already past a step
    /// never runs it -- the property that makes a chain a chain rather than a
    /// set of resets that happen to be idempotent.
    #[test]
    fn a_version_past_a_step_does_not_run_that_step() {
        // `1` is the version this build writes, so a file at `1` skips the v0
        // step and keeps the value that step would have reset.
        let mut one = at(1);
        one.user.sparse_vhd = true;
        assert!(!migrate_config(&mut one));
        assert!(one.user.sparse_vhd, "the v0 step ran on a v1 file");

        // And the same value *is* reset when the file is still at v0.
        let mut zero = at(0);
        zero.user.sparse_vhd = true;
        assert!(migrate_config(&mut zero));
        assert!(!zero.user.sparse_vhd);
    }

    /// The reference's chain ends by stamping the version, and so does this one.
    /// A migration that brought fields forward but left the version at `0` would
    /// run the same steps on every load for the rest of the file's life.
    #[test]
    fn the_version_is_stamped_whatever_the_steps_did() {
        // Every version below this build's, rather than a literal `0`: the test
        // is about the chain's tail, and it should not need editing when a
        // version is added.
        for start in 0..SETTINGS_VERSION {
            let mut config = at(start);
            migrate_config(&mut config);
            assert_eq!(config.application.setting_version, SETTINGS_VERSION);
        }
    }
}
