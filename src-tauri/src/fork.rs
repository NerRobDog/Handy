//! Facts that make this build Handy.satoru rather than upstream Handy. Kept in
//! one module so syncing with upstream touches as few shared lines as possible.

/// Name shown in the Dock, menu bar, window title and tray tooltip. Must
/// match `productName` in `tauri.conf.json`.
pub const PRODUCT_NAME: &str = "Handy.satoru";

/// Whether this binary was built with the self-updater switched off. Fork
/// builds set `HANDY_DISABLE_UPDATER` in `src-tauri/.cargo/config.toml`: the
/// updater endpoint serves upstream releases, which would overwrite
/// Handy.satoru. Accepts the same values as the runtime env flag.
pub fn updater_disabled_at_build() -> bool {
    option_env!("HANDY_DISABLE_UPDATER").is_some_and(|value| {
        !matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "" | "0" | "false" | "no" | "off"
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tauri_conf() -> serde_json::Value {
        serde_json::from_str(include_str!("../tauri.conf.json")).expect("tauri.conf.json parses")
    }

    #[test]
    fn tauri_conf_carries_the_fork_identity() {
        let conf = tauri_conf();
        assert_eq!(conf["productName"], PRODUCT_NAME);
        assert_eq!(conf["identifier"], "io.github.nerrobdog.handy-satoru");
        assert_eq!(conf["bundle"]["createUpdaterArtifacts"], false);
    }

    #[test]
    fn fork_builds_disable_the_updater() {
        assert!(updater_disabled_at_build());
        assert!(crate::settings::update_checks_forced_disabled());
    }
}
