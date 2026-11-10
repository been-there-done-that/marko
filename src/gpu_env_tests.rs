use super::drivers_to_skip;

fn v(ids: &[&str]) -> Vec<String> {
    ids.iter().map(|s| s.to_string()).collect()
}

#[test]
fn no_gpu_keeps_every_driver() {
    assert_eq!(drivers_to_skip(&[]), None);
}

#[test]
fn intel_only_skips_software_and_amd() {
    assert_eq!(drivers_to_skip(&v(&["0x8086"])).as_deref(), Some("*lvp*,*radeon*"));
}

#[test]
fn amd_keeps_radeon() {
    assert_eq!(drivers_to_skip(&v(&["0x8086", "0x1002\n"])).as_deref(), Some("*lvp*"));
}
