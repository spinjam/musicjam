use gpui::{Hsla, WindowAppearance, rgb};

/// Amber marks what the speakers play, blue what the headphones play: the
/// same pairing as the mockup the layout was chosen from.
#[derive(Clone, Copy)]
pub struct Palette {
    pub bg: Hsla,
    pub panel: Hsla,
    pub card: Hsla,
    pub border: Hsla,
    pub text: Hsla,
    pub muted: Hsla,
    pub selected: Hsla,
    pub cue: Hsla,
    pub live: Hsla,
    pub wave: Hsla,
    pub on_accent: Hsla,
    pub warn_bg: Hsla,
}

pub fn palette(appearance: WindowAppearance) -> Palette {
    match appearance {
        WindowAppearance::Dark | WindowAppearance::VibrantDark => Palette {
            bg: rgb(0x14171c).into(),
            panel: rgb(0x1a1e25).into(),
            card: rgb(0x222731).into(),
            border: rgb(0x2e3440).into(),
            text: rgb(0xe6e9ef).into(),
            muted: rgb(0x98a1b2).into(),
            selected: rgb(0x26365e).into(),
            cue: rgb(0x7ea2ff).into(),
            live: rgb(0xf0a44b).into(),
            wave: rgb(0x4a5263).into(),
            on_accent: rgb(0x10131a).into(),
            warn_bg: rgb(0x3d2c14).into(),
        },
        _ => Palette {
            bg: rgb(0xf4f5f8).into(),
            panel: rgb(0xeceef3).into(),
            card: rgb(0xffffff).into(),
            border: rgb(0xd6d9e0).into(),
            text: rgb(0x1d2230).into(),
            muted: rgb(0x6b7385).into(),
            selected: rgb(0xdfe7ff).into(),
            cue: rgb(0x1f4fd6).into(),
            live: rgb(0xb8660b).into(),
            wave: rgb(0xb9c0d0).into(),
            on_accent: rgb(0xffffff).into(),
            warn_bg: rgb(0xfff1dc).into(),
        },
    }
}
