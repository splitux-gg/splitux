// Input device pipelines - orchestration functions

pub mod permissions;

pub use permissions::{check_permissions, install_udev_rules, PermissionStatus};
