//! Records the commit Marko is built from (`MARKO_COMMIT`), and embeds the app icon and version
//! info in the Windows executable.

fn main() {
    println!("cargo:rerun-if-changed=packaging/icons/marko.ico");
    // The short commit hash About Marko shows. Rebuilt when HEAD moves.
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs/heads");
    let commit = std::process::Command::new("git").args(["rev-parse", "--short=7", "HEAD"]).output().ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".into());
    println!("cargo:rustc-env=MARKO_COMMIT={commit}");
    #[cfg(windows)]
    {
        let icon = std::path::Path::new("packaging/icons/marko.ico");
        if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") && icon.exists() {
            let mut res = winresource::WindowsResource::new();
            res.set_icon(&icon.to_string_lossy());
            res.set("ProductName", "Marko");
            res.set("FileDescription", "Marko — Markdown viewer and editor");
            if let Err(e) = res.compile() {
                println!("cargo:warning=could not embed the Windows icon: {e}");
            }
        }
    }
}
