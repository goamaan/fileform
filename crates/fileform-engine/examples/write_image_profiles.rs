// SPDX-License-Identifier: Apache-2.0
//! Generate standard-profile fixtures for independent process QA.
use moxcms::ColorProfile;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let directory = std::path::PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("output directory required")?,
    );
    for (name, profile) in [
        ("srgb", ColorProfile::new_srgb()),
        ("adobe-rgb", ColorProfile::new_adobe_rgb()),
        ("display-p3", ColorProfile::new_display_p3()),
    ] {
        std::fs::write(directory.join(format!("{name}.icc")), profile.encode()?)?;
    }
    Ok(())
}
