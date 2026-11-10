//! Keeps the Vulkan loader from loading drivers this machine cannot use.
//!
//! The loader opens every installed driver to list GPUs. Two of them, lavapipe (software)
//! and radv (AMD), link libLLVM, which adds about 45 MB of resident memory to a process
//! that never draws with them. When a hardware GPU is present we skip lavapipe, and when
//! no AMD GPU is present we skip radv. A user's own loader settings always win.

/// PCI vendor id of AMD GPUs.
const AMD: &str = "0x1002";

/// The `VK_LOADER_DRIVERS_DISABLE` value for these GPU vendor ids, or `None` to leave the
/// loader alone (no GPU found, so software rendering may be the only way to draw).
#[cfg_attr(not(target_os = "linux"), allow(dead_code, reason = "only Linux uses the Vulkan loader"))]
pub fn drivers_to_skip(vendors: &[String]) -> Option<String> {
    if vendors.is_empty() {
        return None;
    }
    let mut skip = vec!["*lvp*"];
    if !vendors.iter().any(|v| v.trim().eq_ignore_ascii_case(AMD)) {
        skip.push("*radeon*");
    }
    Some(skip.join(","))
}

/// Sets `VK_LOADER_DRIVERS_DISABLE` unless the user already steers the loader.
///
/// # Safety
/// Call before any other thread starts: it writes the process environment.
#[cfg(target_os = "linux")]
pub unsafe fn apply() {
    const USER_VARS: [&str; 4] = [
        "VK_LOADER_DRIVERS_DISABLE",
        "VK_LOADER_DRIVERS_SELECT",
        "VK_DRIVER_FILES",
        "VK_ICD_FILENAMES",
    ];
    if USER_VARS.iter().any(|v| std::env::var_os(v).is_some()) {
        return;
    }
    if let Some(skip) = drivers_to_skip(&gpu_vendors()) {
        // SAFETY: the caller guarantees no other thread is running.
        unsafe { std::env::set_var("VK_LOADER_DRIVERS_DISABLE", skip) };
    }
}

/// PCI vendor ids of the display devices under /sys/class/drm.
#[cfg(target_os = "linux")]
fn gpu_vendors() -> Vec<String> {
    let Ok(dir) = std::fs::read_dir("/sys/class/drm") else { return Vec::new() };
    dir.flatten()
        .filter(|e| {
            let name = e.file_name();
            let name = name.to_string_lossy();
            name.starts_with("card") && !name.contains('-')
        })
        .filter_map(|e| std::fs::read_to_string(e.path().join("device/vendor")).ok())
        .map(|v| v.trim().to_string())
        .collect()
}

#[cfg(test)]
#[path = "gpu_env_tests.rs"]
mod tests;
