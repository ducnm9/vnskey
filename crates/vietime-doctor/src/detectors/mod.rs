// SPDX-License-Identifier: GPL-3.0-or-later
//
// Concrete detectors shipped with Doctor v0.1.
//
// Spec ref: `spec/01-phase1-doctor.md` §B.3.

pub mod app_electron;
pub mod app_generic;
pub mod desktop;
pub mod distro;
pub mod env_etc_environment;
pub mod env_etc_profile_d;
pub mod env_home_profile;
pub mod env_process;
pub mod env_systemd;
pub mod fcitx5_config;
pub mod fcitx5_daemon;
pub mod ibus_daemon;
pub mod ibus_engines;
pub mod package_engines;
pub mod session;

pub use app_electron::ElectronAppDetector;
pub use app_generic::GenericAppDetector;
pub use desktop::DesktopDetector;
pub use distro::DistroDetector;
pub use env_etc_environment::EtcEnvironmentDetector;
pub use env_etc_profile_d::EtcProfileDDetector;
pub use env_home_profile::HomeProfileDetector;
pub use env_process::ProcessEnvDetector;
pub use env_systemd::SystemdEnvDetector;
pub use fcitx5_config::Fcitx5ConfigDetector;
pub use fcitx5_daemon::Fcitx5DaemonDetector;
pub use ibus_daemon::IbusDaemonDetector;
pub use ibus_engines::IbusEnginesDetector;
pub use package_engines::PackageEnginesDetector;
pub use session::SessionDetector;
