//! Embeds the app icon and version info in the Windows executable. Does nothing elsewhere.

fn main() {
    println!("cargo:rerun-if-changed=packaging/icons/marko.ico");
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
