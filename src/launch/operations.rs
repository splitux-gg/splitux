//! Operations module (atomic side effects)

pub mod overlays;
pub mod profiles;

pub use overlays::fuse_overlayfs_mount_gamedirs;
pub use profiles::setup_profiles;
