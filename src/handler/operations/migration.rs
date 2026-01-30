// Legacy backend migration (reads/transforms handler data)

use crate::backend::{
    FacepunchSettings as BackendFacepunchSettings,
    GoldbergSettings as BackendGoldbergSettings,
    MultiplayerBackend,
    PhotonSettings as BackendPhotonSettings,
};
use crate::handler::Handler;

/// Migrate legacy backend fields to new optional backend format
pub fn migrate_legacy_backends(handler: &mut Handler) {
    // Migrate Goldberg: old enum + flat fields -> new Optional<GoldbergSettings>
    if handler.goldberg.is_none() {
        let should_enable = handler.backend == MultiplayerBackend::Goldberg
            || handler.use_goldberg
            || !handler.goldberg_settings.is_empty()
            || handler.goldberg_disable_networking
            || handler.goldberg_networking_sockets;

        if should_enable {
            handler.goldberg = Some(BackendGoldbergSettings {
                disable_networking: handler.goldberg_disable_networking,
                networking_sockets: handler.goldberg_networking_sockets,
                settings: handler.goldberg_settings.clone(),
                plugin: None,
            });
        }
    }

    // Migrate Photon: old enum + struct -> new Optional<PhotonSettings>
    if handler.photon.is_none() {
        let should_enable = handler.backend == MultiplayerBackend::Photon
            || !handler.photon_settings.is_empty();

        if should_enable {
            handler.photon = Some(BackendPhotonSettings {
                config_path: handler.photon_settings.config_path.clone(),
                shared_files: handler.photon_settings.shared_files.clone(),
                plugin: None,
            });
        }
    }

    // Migrate Facepunch: presence-based -> new Optional<FacepunchSettings>
    if handler.facepunch.is_none() {
        let should_enable = !handler.facepunch_settings.is_default()
            || !handler.runtime_patches.is_empty();

        if should_enable {
            handler.facepunch = Some(BackendFacepunchSettings {
                spoof_identity: handler.facepunch_settings.spoof_identity,
                force_valid: handler.facepunch_settings.force_valid,
                photon_bypass: handler.facepunch_settings.photon_bypass,
            });
        }
    }

    // Clear deprecated fields after migration
    handler.use_goldberg = false;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn no_legacy_fields_no_migration() {
        let mut h = Handler::default();
        migrate_legacy_backends(&mut h);
        assert!(h.goldberg.is_none());
        assert!(h.photon.is_none());
        assert!(h.facepunch.is_none());
    }

    #[test]
    fn use_goldberg_triggers_migration() {
        let mut h = Handler::default();
        h.use_goldberg = true;
        migrate_legacy_backends(&mut h);
        assert!(h.goldberg.is_some());
        assert!(!h.use_goldberg, "use_goldberg should be cleared");
    }

    #[test]
    fn backend_goldberg_triggers_migration() {
        let mut h = Handler::default();
        h.backend = MultiplayerBackend::Goldberg;
        migrate_legacy_backends(&mut h);
        assert!(h.goldberg.is_some());
    }

    #[test]
    fn goldberg_disable_networking_migrated() {
        let mut h = Handler::default();
        h.goldberg_disable_networking = true;
        migrate_legacy_backends(&mut h);
        let gb = h.goldberg.as_ref().unwrap();
        assert!(gb.disable_networking);
        assert!(!gb.networking_sockets);
    }

    #[test]
    fn goldberg_settings_entries_migrated() {
        let mut h = Handler::default();
        h.goldberg_settings.insert("force_lobby_type.txt".into(), "2".into());
        migrate_legacy_backends(&mut h);
        let gb = h.goldberg.as_ref().unwrap();
        assert_eq!(gb.settings.get("force_lobby_type.txt").unwrap(), "2");
    }

    #[test]
    fn backend_photon_triggers_migration() {
        let mut h = Handler::default();
        h.backend = MultiplayerBackend::Photon;
        migrate_legacy_backends(&mut h);
        assert!(h.photon.is_some());
    }

    #[test]
    fn existing_goldberg_not_overwritten() {
        let mut h = Handler::default();
        let existing = BackendGoldbergSettings {
            disable_networking: true,
            networking_sockets: true,
            settings: HashMap::new(),
            plugin: None,
        };
        h.goldberg = Some(existing);
        // Set legacy fields that would normally trigger migration
        h.use_goldberg = true;
        h.goldberg_disable_networking = false;
        migrate_legacy_backends(&mut h);
        let gb = h.goldberg.as_ref().unwrap();
        // Should retain original values, not legacy ones
        assert!(gb.disable_networking);
        assert!(gb.networking_sockets);
    }

    #[test]
    fn multiple_backends_triggered_simultaneously() {
        let mut h = Handler::default();
        h.use_goldberg = true;
        h.backend = MultiplayerBackend::Photon;
        migrate_legacy_backends(&mut h);
        assert!(h.goldberg.is_some());
        assert!(h.photon.is_some());
    }

    #[test]
    fn use_goldberg_always_cleared() {
        let mut h = Handler::default();
        h.use_goldberg = true;
        migrate_legacy_backends(&mut h);
        assert!(!h.use_goldberg);

        // Also cleared when no migration happens
        let mut h2 = Handler::default();
        h2.use_goldberg = false;
        migrate_legacy_backends(&mut h2);
        assert!(!h2.use_goldberg);
    }
}
