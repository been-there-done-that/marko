use super::{snap, value_at};

#[test]
fn value_at_maps_the_rail_linearly() {
    assert_eq!(value_at(0.0, 200.0, 12.0, 24.0, 1.0), 12.0);
    assert_eq!(value_at(100.0, 200.0, 12.0, 24.0, 1.0), 18.0);
    assert_eq!(value_at(200.0, 200.0, 12.0, 24.0, 1.0), 24.0);
}

#[test]
fn value_at_snaps_to_the_step() {
    // 0.37 of the way across 12..24 is 16.44, which snaps to 16.
    assert_eq!(value_at(74.0, 200.0, 12.0, 24.0, 1.0), 16.0);
    assert_eq!(value_at(74.0, 200.0, 1.2, 2.2, 0.1), 1.6);
}

#[test]
fn value_at_clamps_outside_the_rail() {
    assert_eq!(value_at(-40.0, 200.0, 0.0, 40.0, 1.0), 0.0);
    assert_eq!(value_at(900.0, 200.0, 0.0, 40.0, 1.0), 40.0);
    assert_eq!(value_at(10.0, 0.0, 0.0, 40.0, 1.0), 0.0);
}

#[test]
fn snap_removes_float_noise_and_clamps() {
    assert_eq!(snap(1.2 + 0.1 * 3.0, 1.2, 2.2, 0.1), 1.5);
    assert_eq!(snap(99.0, 12.0, 24.0, 1.0), 24.0);
}
