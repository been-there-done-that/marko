//! Marko's version and the commit it was built from, for About Marko and Settings → General.

/// `Marko 0.1.1 (abc1234)`: the version from Cargo.toml and the short commit hash `build.rs`
/// read from git, or `unknown` when it was built outside a git checkout.
pub fn label() -> String {
    format!("Marko {} ({})", env!("CARGO_PKG_VERSION"), env!("MARKO_COMMIT"))
}

#[cfg(test)]
#[path = "version_tests.rs"]
mod tests;
