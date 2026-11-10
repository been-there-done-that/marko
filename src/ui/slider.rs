//! A horizontal slider (rune-kit has none): a rail, an accent fill, a round thumb and the value
//! as text. Drag anywhere on the rail, or focus it and use the arrow keys.

use std::rc::Rc;

use rune::{Color, El, Phase, Tx, el, row, text};
use rune_kit::Kit;

/// Width of the rail in points. The drag maths reads the element's own width, so this only
/// sets how wide it draws.
pub const RAIL_W: f32 = 164.0;
const THUMB: f32 = 16.0;
const HIT_H: f32 = 24.0;
const VALUE_W: f32 = 56.0;

/// The value a press at `x` (of a rail `w` points wide) selects: linear between `min` and `max`,
/// snapped to `step` from `min`, and clamped to the range.
pub fn value_at(x: f32, w: f32, min: f32, max: f32, step: f32) -> f32 {
    let t = if w > 0.0 { (x / w).clamp(0.0, 1.0) } else { 0.0 };
    snap(min + t * (max - min), min, max, step)
}

/// `v` moved onto the `step` grid that starts at `min`, within `min..=max`.
pub fn snap(v: f32, min: f32, max: f32, step: f32) -> f32 {
    let snapped = if step > 0.0 { min + ((v - min) / step).round() * step } else { v };
    // Strip the float noise a step like 0.1 leaves behind.
    ((snapped.clamp(min, max)) * 1000.0).round() / 1000.0
}

type Change<S> = Rc<dyn Fn(&mut S, f32)>;

pub struct Slider<S> {
    key: String,
    value: f32,
    min: f32,
    max: f32,
    step: f32,
    label: Option<String>,
    format: fn(f32) -> String,
    disabled: bool,
    change: Option<Change<S>>,
}

impl<S: 'static> Slider<S> {
    pub fn new(key: impl Into<String>, value: f32, min: f32, max: f32, step: f32) -> Self {
        Self { key: key.into(), value, min, max, step, label: None, format: |v| format!("{v}"), disabled: false, change: None }
    }

    /// The accessible name.
    pub fn label(mut self, l: impl Into<String>) -> Self {
        self.label = Some(l.into());
        self
    }

    /// How the value reads on the right.
    pub fn format(mut self, f: fn(f32) -> String) -> Self {
        self.format = f;
        self
    }

    /// Drawn at 40% with no handlers.
    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }

    pub fn on_change(mut self, f: impl Fn(&mut S, f32) + 'static) -> Self {
        self.change = Some(Rc::new(f));
        self
    }

    /// The slider with its value text, as one row. `accent` fills the elapsed part of the rail.
    pub fn build(self, k: &Kit, accent: Color) -> El {
        let c = &k.c;
        let span = (self.max - self.min).max(f32::EPSILON);
        let frac = ((self.value - self.min) / span).clamp(0.0, 1.0);
        let x = RAIL_W * frac;
        let rail = el().absolute(0.0, (HIT_H - 4.0) / 2.0).size(RAIL_W, 4.0).radius(2.0).fill(c.border_base);
        let fill = el().absolute(0.0, (HIT_H - 4.0) / 2.0).size(x.max(0.0), 4.0).radius(2.0).fill(accent);
        let thumb = el().key("thumb").absolute(x - THUMB / 2.0, (HIT_H - THUMB) / 2.0).size(THUMB, THUMB).radius(THUMB / 2.0)
            .fill(Color::WHITE).border(1.0, c.border_strong);
        let mut track = el().key(self.key.clone()).size(RAIL_W, HIT_H).role("slider")
            .range_value(f64::from(self.value), f64::from(self.min), f64::from(self.max)).range_step(f64::from(self.step))
            .child(rail).child(fill).child(thumb);
        if let Some(l) = &self.label {
            track = track.label(l.clone());
        }
        let readout = row().w(VALUE_W).justify_end().child(text((self.format)(self.value)).font(k.ty.ui).color(c.text_base));
        let group = row().gap(12.0).center_y();
        if self.disabled {
            return group.child(track.disabled(true)).child(readout).layer_opacity(0.4);
        }
        if let Some(change) = self.change {
            let (min, max, step, value) = (self.min, self.max, self.step, self.value);
            let drag = change.clone();
            let stepper = change;
            track = track
                .on_drag(move |s: &mut S, d: &mut rune::Drag, _: &mut Tx| {
                    if d.cancelled() && matches!(d.phase, Phase::Up) {
                        return;
                    }
                    drag(s, value_at(d.x, d.w, min, max, step));
                })
                .on_step(move |s: &mut S, delta: i32, _: &mut Tx| stepper(s, snap(value + delta as f32 * step, min, max, step)))
                .cursor(rune::Cursor::Pointer);
        }
        group.child(track).child(readout)
    }
}

#[cfg(test)]
#[path = "slider_tests.rs"]
mod tests;
