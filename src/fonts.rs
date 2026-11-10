//! The installed font families the Settings panel offers. Filled after launch by the font
//! loader (`fonts::load`, off the UI thread); empty until then.

/// Installed families, sorted, without hidden (`.`-prefixed) ones.
#[derive(Default, Debug, Clone, PartialEq)]
pub struct FontList {
    /// Every installed family.
    pub all: Vec<String>,
    /// The monospaced ones, for the code font.
    pub mono: Vec<String>,
    /// True once the loader has answered (the lists may still be empty).
    pub loaded: bool,
}

/// Turns `fc-list --format='%{family}\n'` output into a family list: the text before the first
/// comma of each line (a line may name several languages' spellings), trimmed, without empty or
/// `.`-prefixed (hidden) names, de-duplicated and sorted without regard to case.
pub fn parse_fc_list(out: &str) -> Vec<String> {
    let mut families: Vec<String> = out
        .lines()
        .filter_map(|line| {
            let name = line.split(',').next().unwrap_or_default().trim();
            (!name.is_empty() && !name.starts_with('.')).then(|| name.to_string())
        })
        .collect();
    families.sort_by_key(|f| f.to_lowercase());
    families.dedup_by_key(|f| f.to_lowercase());
    families
}

/// Runs `fc-list` with `args` and parses its families; empty when it is missing or fails.
#[cfg(target_os = "linux")]
fn fc_list(args: &[&str]) -> Vec<String> {
    let output = std::process::Command::new("fc-list").args(args).arg("--format=%{family}\\n").output();
    match output {
        Ok(o) if o.status.success() => parse_fc_list(&String::from_utf8_lossy(&o.stdout)),
        _ => Vec::new(),
    }
}

/// Asks the system which families are installed. Blocking: run it off the UI thread. Linux asks
/// fontconfig; other systems return empty lists (the pickers then offer only the defaults).
pub fn load() -> FontList {
    #[cfg(target_os = "linux")]
    {
        let all = fc_list(&[]);
        let mut mono = fc_list(&[":spacing=mono"]);
        mono.extend(fc_list(&[":spacing=100"]));
        mono.sort_by_key(|f| f.to_lowercase());
        mono.dedup_by_key(|f| f.to_lowercase());
        FontList { all, mono, loaded: true }
    }
    #[cfg(not(target_os = "linux"))]
    {
        FontList { loaded: true, ..FontList::default() }
    }
}

#[cfg(test)]
#[path = "fonts_tests.rs"]
mod tests;
