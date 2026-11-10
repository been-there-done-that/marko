//! The full-size view of a document image: a dark scrim with the picture fitted inside it.
//! A press anywhere, or Esc, closes it.

use rune::{El, SurfaceFit, col, el};
use rune_kit::ImageFrame;

use crate::app::{AppState, Lightbox};

/// Makes a picture in the document open the full-size view when pressed.
pub fn open_on_press(image: El, frame: &ImageFrame<'_>) -> El {
    let picture = frame.image.clone();
    let alt = frame.alt.to_string();
    // The Markdown renderer keys what this returns, so the key goes on an inner wrapper.
    let inner = col().key("doc-image").w_full().child(image.on_press(move |s: &mut AppState| {
        s.lightbox = Some(Lightbox { image: picture.clone(), alt: alt.clone() });
    }));
    col().w_full().child(inner)
}

/// Adds the viewer to `layers` while a picture is open.
pub fn layer(s: &AppState, layers: &mut Vec<El>) {
    let Some(open) = &s.lightbox else { return };
    let label = if open.alt.is_empty() { "Image preview".to_string() } else { open.alt.clone() };
    let picture = el().role("image").label(label).w_full().h_full().pixels_fit(open.image.pixels(), SurfaceFit::Fit);
    let scrim = col().key("lightbox").pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
        .fill(rune::Color::BLACK.alpha(0.82)).center().padding(40.0)
        .on_press(|s: &mut AppState| s.lightbox = None)
        .child(picture);
    layers.push(scrim);
}
