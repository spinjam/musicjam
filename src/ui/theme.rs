//! Colour themes. Every theme names two deck colours: `live` is what the
//! speakers play, `cue` what the headphones play, the way a DJ mixer colours
//! its two channels. The rest is the booth: near-black planes, hard edges.

use gpui::{App, Hsla, WindowAppearance, rgb};

pub const CHROME_FONT: &str = "IBM Plex Sans";
/// Labels, times and anything that reads like a hardware display.
pub const DISPLAY_FONT: &str = "Lilex";

/// Square corners: controls, not cards.
pub const RADIUS: f32 = 2.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Appearance {
    Dark,
    Light,
}

impl Appearance {
    pub fn of_system(a: WindowAppearance) -> Self {
        match a {
            WindowAppearance::Dark | WindowAppearance::VibrantDark => Self::Dark,
            _ => Self::Light,
        }
    }
}

/// A theme as sRGB hex, so the contrast test reads the same numbers the
/// screen gets.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Theme {
    pub name: &'static str,
    pub appearance: Appearance,
    /// Furthest back: the window, the deck column.
    pub bg: u32,
    /// The library tree.
    pub panel: u32,
    /// The track list and the decks: what is worked on.
    pub card: u32,
    pub border: u32,
    pub text: u32,
    pub muted: u32,
    pub selected: u32,
    pub live: u32,
    pub cue: u32,
    /// Ink on a filled deck colour.
    pub on_accent: u32,
    pub warn_bg: u32,
}

impl Theme {
    /// Dark first, then light: the order the picker shows.
    pub fn all() -> Vec<Theme> {
        vec![
            Self::club(),
            Self::neon(),
            Self::acid(),
            Self::booth(),
            Self::vinyl(),
            Self::ice(),
            Self::gruvbox_dark(),
            Self::catppuccin_mocha(),
            Self::tokyo_night(),
            Self::dracula(),
            Self::daylight(),
            Self::gruvbox_light(),
            Self::catppuccin_latte(),
        ]
    }

    pub fn club() -> Self {
        Self {
            name: "Club",
            appearance: Appearance::Dark,
            bg: 0x08080a,
            panel: 0x0f0f12,
            card: 0x151519,
            border: 0x2a2a31,
            text: 0xf2f2f5,
            muted: 0x9a9aa6,
            selected: 0x1f2b3d,
            live: 0xff8a00,
            cue: 0x00d2ff,
            on_accent: 0x08080a,
            warn_bg: 0x3a2200,
        }
    }

    fn neon() -> Self {
        Self {
            name: "Neon",
            bg: 0x0a0512,
            panel: 0x10091d,
            card: 0x170e2a,
            border: 0x34205a,
            text: 0xf5efff,
            muted: 0xa898c8,
            selected: 0x2b1a4d,
            live: 0xff2bd6,
            cue: 0x29f0ff,
            on_accent: 0x0a0512,
            warn_bg: 0x3d0f33,
            ..Self::club()
        }
    }

    fn acid() -> Self {
        Self {
            name: "Acid",
            bg: 0x070907,
            panel: 0x0d110d,
            card: 0x131913,
            border: 0x263226,
            text: 0xeef5ea,
            muted: 0x93a58e,
            selected: 0x1c2c1c,
            live: 0xc6ff00,
            cue: 0xff3d7f,
            on_accent: 0x070907,
            warn_bg: 0x2a2a00,
            ..Self::club()
        }
    }

    fn booth() -> Self {
        Self {
            name: "Booth",
            bg: 0x131518,
            panel: 0x191c20,
            card: 0x20242a,
            border: 0x343a42,
            text: 0xe8ebef,
            muted: 0x9aa3ae,
            selected: 0x263a52,
            live: 0xff5a1f,
            cue: 0x3d9bff,
            on_accent: 0x0b0c0e,
            warn_bg: 0x3a2412,
            ..Self::club()
        }
    }

    fn vinyl() -> Self {
        Self {
            name: "Vinyl",
            bg: 0x100a08,
            panel: 0x18110e,
            card: 0x211713,
            border: 0x3b2a22,
            text: 0xf6ece4,
            muted: 0xb39d8f,
            selected: 0x3a2418,
            live: 0xff4136,
            cue: 0xffc542,
            on_accent: 0x100a08,
            warn_bg: 0x3d1f10,
            ..Self::club()
        }
    }

    fn ice() -> Self {
        Self {
            name: "Ice",
            bg: 0x050a11,
            panel: 0x0a121e,
            card: 0x101b2a,
            border: 0x22344c,
            text: 0xeaf4ff,
            muted: 0x8ea6c2,
            selected: 0x16304f,
            live: 0x4dfff3,
            cue: 0xb39bff,
            on_accent: 0x050a11,
            warn_bg: 0x2b2440,
            ..Self::club()
        }
    }

    fn gruvbox_dark() -> Self {
        Self {
            name: "Gruvbox Dark",
            bg: 0x1d2021,
            panel: 0x232627,
            card: 0x282828,
            border: 0x504945,
            text: 0xebdbb2,
            muted: 0xa89984,
            selected: 0x45403d,
            live: 0xfe8019,
            cue: 0x83a598,
            on_accent: 0x1d2021,
            warn_bg: 0x3c2f12,
            ..Self::club()
        }
    }

    fn catppuccin_mocha() -> Self {
        Self {
            name: "Catppuccin Mocha",
            bg: 0x11111b,
            panel: 0x181825,
            card: 0x1e1e2e,
            border: 0x313244,
            text: 0xcdd6f4,
            muted: 0x9399b2,
            selected: 0x313244,
            live: 0xfab387,
            cue: 0x89b4fa,
            on_accent: 0x11111b,
            warn_bg: 0x3a2a1e,
            ..Self::club()
        }
    }

    fn tokyo_night() -> Self {
        Self {
            name: "Tokyo Night",
            bg: 0x16161e,
            panel: 0x1a1b26,
            card: 0x1f2335,
            border: 0x2f3549,
            text: 0xc0caf5,
            muted: 0x9aa5ce,
            selected: 0x283457,
            live: 0xff9e64,
            cue: 0x7aa2f7,
            on_accent: 0x16161e,
            warn_bg: 0x3b2a1f,
            ..Self::club()
        }
    }

    fn dracula() -> Self {
        Self {
            name: "Dracula",
            bg: 0x191a21,
            panel: 0x21222c,
            card: 0x282a36,
            border: 0x44475a,
            text: 0xf8f8f2,
            muted: 0xb0b4cf,
            selected: 0x44475a,
            live: 0xffb86c,
            cue: 0x8be9fd,
            on_accent: 0x191a21,
            warn_bg: 0x40301f,
            ..Self::club()
        }
    }

    pub fn daylight() -> Self {
        Self {
            name: "Daylight",
            appearance: Appearance::Light,
            bg: 0xe9ebf0,
            panel: 0xf1f2f5,
            card: 0xffffff,
            border: 0xc9cdd6,
            text: 0x111318,
            muted: 0x585f6e,
            selected: 0xd6e0ff,
            live: 0xc25400,
            cue: 0x1f4fd6,
            on_accent: 0xffffff,
            warn_bg: 0xffe9c7,
        }
    }

    fn gruvbox_light() -> Self {
        Self {
            name: "Gruvbox Light",
            bg: 0xf2e5bc,
            panel: 0xf9f5d7,
            card: 0xfbf1c7,
            border: 0xd5c4a1,
            text: 0x282828,
            muted: 0x665c54,
            selected: 0xebdbb2,
            live: 0xaf3a03,
            cue: 0x076678,
            ..Self::daylight()
        }
    }

    fn catppuccin_latte() -> Self {
        Self {
            name: "Catppuccin Latte",
            bg: 0xe6e9ef,
            panel: 0xeff1f5,
            card: 0xf8f9fb,
            border: 0xccd0da,
            text: 0x4c4f69,
            muted: 0x5c5f77,
            selected: 0xdce0e8,
            live: 0xb84a00,
            cue: 0x1e66f5,
            ..Self::daylight()
        }
    }

    pub fn palette(&self) -> Palette {
        let c = |hex: u32| -> Hsla { rgb(hex).into() };
        Palette {
            bg: c(self.bg),
            panel: c(self.panel),
            card: c(self.card),
            border: c(self.border),
            text: c(self.text),
            muted: c(self.muted),
            selected: c(self.selected),
            live: c(self.live),
            cue: c(self.cue),
            live_dim: c(mix(self.live, self.card, 0.30)),
            cue_dim: c(mix(self.cue, self.card, 0.30)),
            wave: c(mix(self.muted, self.card, 0.35)),
            on_accent: c(self.on_accent),
            warn_bg: c(self.warn_bg),
        }
    }

    /// Hands the theme to gpui-component, which draws the search field and
    /// the sliders.
    pub fn apply_to_components(&self, cx: &mut App) {
        let p = self.palette();
        let mode = match self.appearance {
            Appearance::Dark => gpui_component::theme::ThemeMode::Dark,
            Appearance::Light => gpui_component::theme::ThemeMode::Light,
        };
        gpui_component::Theme::change(mode, None, cx);
        gpui_component::Theme::update(cx, |t| {
            t.shadow = false;
            t.radius = gpui::px(RADIUS);
            t.radius_lg = gpui::px(RADIUS);
            t.font_family = CHROME_FONT.into();
            t.colors.background = p.card;
            t.colors.foreground = p.text;
            t.colors.input = p.border;
            t.colors.border = p.border;
            t.colors.caret = p.cue;
            t.colors.selection = p.selected;
            t.colors.ring = p.cue;
            t.colors.muted = p.panel;
            t.colors.muted_foreground = p.muted;
            t.colors.popover = p.card;
            t.colors.popover_foreground = p.text;
            t.colors.slider_bar = p.text;
            t.colors.slider_thumb = p.text;
        });
    }
}

/// The resolved colours views paint with.
#[derive(Clone, Copy)]
pub struct Palette {
    pub bg: Hsla,
    pub panel: Hsla,
    pub card: Hsla,
    pub border: Hsla,
    pub text: Hsla,
    pub muted: Hsla,
    pub selected: Hsla,
    pub live: Hsla,
    pub cue: Hsla,
    /// The unplayed part of a deck's waveform.
    pub live_dim: Hsla,
    pub cue_dim: Hsla,
    pub wave: Hsla,
    pub on_accent: Hsla,
    pub warn_bg: Hsla,
}

/// `a` over `b` at `amount` (0 = all b, 1 = all a), in sRGB.
fn mix(a: u32, b: u32, amount: f32) -> u32 {
    let ch = |c: u32, shift: u32| ((c >> shift) & 0xff) as f32;
    [16, 8, 0].iter().fold(0, |acc, &s| {
        let v = ch(a, s) * amount + ch(b, s) * (1.0 - amount);
        acc | ((v.round() as u32) << s)
    })
}

pub fn find(name: Option<&str>, system: Appearance) -> Theme {
    name.and_then(|n| Theme::all().into_iter().find(|t| t.name == n))
        .unwrap_or_else(|| match system {
            Appearance::Dark => Theme::club(),
            Appearance::Light => Theme::daylight(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn luminance(hex: u32) -> f32 {
        let lin = |c: u32| {
            let v = c as f32 / 255.0;
            if v <= 0.04045 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * lin((hex >> 16) & 0xff)
            + 0.7152 * lin((hex >> 8) & 0xff)
            + 0.0722 * lin(hex & 0xff)
    }

    fn contrast(a: u32, b: u32) -> f32 {
        let (x, y) = (luminance(a), luminance(b));
        (x.max(y) + 0.05) / (x.min(y) + 0.05)
    }

    #[test]
    fn every_theme_holds_its_text() {
        for t in Theme::all() {
            let check = |what: &str, fg: u32, bg: u32, min: f32| {
                let r = contrast(fg, bg);
                assert!(r >= min, "{}: {what} is {r:.2}:1, under {min}", t.name);
            };
            for plane in [t.bg, t.panel, t.card] {
                check("text", t.text, plane, 4.5);
                check("muted text", t.muted, plane, 3.0);
                check("live", t.live, plane, 3.0);
                check("cue", t.cue, plane, 3.0);
            }
            check("text on selection", t.text, t.selected, 4.5);
            check("text on a warning", t.text, t.warn_bg, 4.5);
            check("ink on live", t.on_accent, t.live, 3.0);
            check("ink on cue", t.on_accent, t.cue, 3.0);
        }
    }

    #[test]
    fn themes_are_found_by_name_and_dark_comes_first() {
        let all = Theme::all();
        for t in &all {
            assert_eq!(find(Some(t.name), Appearance::Light).name, t.name);
        }
        let first_light = all
            .iter()
            .position(|t| t.appearance == Appearance::Light)
            .unwrap();
        assert!(
            all[first_light..]
                .iter()
                .all(|t| t.appearance == Appearance::Light)
        );
        assert_eq!(find(Some("gone"), Appearance::Dark).name, "Club");
    }

    #[test]
    fn mix_blends_channels() {
        assert_eq!(mix(0xffffff, 0x000000, 0.5), 0x808080);
        assert_eq!(mix(0xff0000, 0x0000ff, 1.0), 0xff0000);
    }
}
