// SPDX-License-Identifier: GPL-3.0-or-later
//
// Concrete detectors shipped with Doctor v0.1.
//
// Spec ref: `spec/01-phase1-doctor.md` §B.3.

pub mod desktop;
pub mod distro;
pub mod session;

pub use desktop::DesktopDetector;
pub use distro::DistroDetector;
pub use session::SessionDetector;
