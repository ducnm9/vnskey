// SPDX-License-Identifier: GPL-3.0-or-later
//
// Concrete detectors shipped with Doctor v0.1.
//
// Spec ref: `spec/01-phase1-doctor.md` §B.3.

pub mod desktop;
pub mod distro;
pub mod env_etc_environment;
pub mod env_etc_profile_d;
pub mod env_home_profile;
pub mod env_process;
pub mod env_systemd;
pub mod session;

pub use desktop::DesktopDetector;
pub use distro::DistroDetector;
pub use env_etc_environment::EtcEnvironmentDetector;
pub use env_etc_profile_d::EtcProfileDDetector;
pub use env_home_profile::HomeProfileDetector;
pub use env_process::ProcessEnvDetector;
pub use env_systemd::{EnvCommand, SystemctlEnvCommand, SystemdEnvDetector};
pub use session::SessionDetector;
