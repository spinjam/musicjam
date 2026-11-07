use std::ops::Range;
use std::sync::Arc;

use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, ClickEvent, Context, Div, ElementId, Entity, FontWeight, Hsla, InteractiveElement,
    IntoElement, MouseButton, ParentElement, Render, SharedString, Stateful,
    StatefulInteractiveElement, Styled, Window, div, px, uniform_list,
};
use gpui_component::input::Input;
use gpui_component::resizable::{h_resizable, resizable_panel};
use gpui_component::slider::{Slider, SliderState};

use super::theme::{CHROME_FONT, DISPLAY_FONT, Palette, RADIUS, Theme};
use super::{Deck, SettingsTab, WAVE_BARS, Workspace};
use crate::actions::*;
use crate::audio::engine::{Command, DeckInfo, Role};

const TREE_WIDTH: f32 = 230.;
const TREE_RANGE: (f32, f32) = (150., 480.);
const DECKS_WIDTH: f32 = 380.;
const DECKS_RANGE: (f32, f32) = (320., 640.);
const ROW_HEIGHT: f32 = 30.;
const WAVE_HEIGHT: f32 = 64.;

/// The shortcuts tab: the same keys `actions::bindings` binds.
const KEYS: [(&str, &str); 12] = [
    ("⌘F", "Cerca"),
    ("↑ ↓", "Scegli un brano"),
    ("Invio / doppio clic", "Carica nel deck B"),
    ("⌘P", "Play / pausa del deck A"),
    ("⌘.", "Stop"),
    ("⌘E", "Anteprima in cuffia (cue)"),
    ("⌘Invio", "Crossfade A → B"),
    ("⌥← ⌥→", "Punto di partenza ±1 s"),
    ("⌥⇧← ⌥⇧→", "Punto di partenza ±0,1 s"),
    ("⌘R", "Aggiorna la libreria"),
    ("⌘T", "Tema successivo"),
    ("⌘,  /  Esc", "Apri / chiudi le impostazioni"),
];

fn clock(secs: f32) -> String {
    let s = secs.max(0.0) as u32;
    format!("{}:{:02}", s / 60, s % 60)
}

/// Small caps of the booth: every label reads like it is printed on a mixer.
fn caps(text: impl Into<SharedString>, color: Hsla) -> Div {
    div()
        .font_family(DISPLAY_FONT)
        .text_size(px(10.5))
        .font_weight(FontWeight::BOLD)
        .text_color(color)
        .child(text.into())
}

fn button(id: impl Into<ElementId>, label: impl Into<SharedString>, p: Palette) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .px_3()
        .h(px(30.))
        .rounded(px(RADIUS))
        .border_1()
        .border_color(p.border)
        .bg(p.panel)
        .font_family(DISPLAY_FONT)
        .text_size(px(11.5))
        .font_weight(FontWeight::BOLD)
        .cursor_pointer()
        .hover(|s| s.border_color(p.text))
        .active(|s| s.bg(p.selected))
        .child(label.into())
}

fn filled(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    color: Hsla,
    p: Palette,
) -> Stateful<Div> {
    button(id, label, p)
        .bg(color)
        .border_color(color)
        .text_color(p.on_accent)
}

fn volume_row(slider: &Entity<SliderState>, level: f32, p: Palette) -> Div {
    div()
        .flex()
        .items_center()
        .gap_3()
        .child(caps("VOL", p.muted).w(px(30.)))
        .child(div().flex_1().child(Slider::new(slider)))
        .child(
            div()
                .w(px(40.))
                .text_right()
                .font_family(DISPLAY_FONT)
                .text_size(px(11.5))
                .text_color(p.muted)
                .child(format!("{:.0}%", level * 100.)),
        )
}

impl Render for Workspace {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = self.theme.palette();
        div()
            .id("workspace")
            .key_context(WORKSPACE)
            .track_focus(&self.focus)
            .relative()
            .size_full()
            .flex()
            .bg(p.bg)
            .text_color(p.text)
            .font_family(CHROME_FONT)
            .text_size(px(13.))
            .on_action(cx.listener(|_, _: &Quit, _, cx| cx.quit()))
            .on_action(cx.listener(|this, _: &OpenSettings, _, cx| this.open_settings(cx)))
            .on_action(cx.listener(|this, _: &Dismiss, window, cx| {
                if this.show_settings {
                    this.close_settings(window, cx);
                } else {
                    this.alerts.clear();
                    cx.notify();
                }
            }))
            .on_action(cx.listener(|this, _: &FocusSearch, window, cx| {
                this.search.update(cx, |s, cx| s.focus(window, cx));
            }))
            .on_action(cx.listener(|this, _: &SelectPrevious, _, cx| this.move_cursor(-1, cx)))
            .on_action(cx.listener(|this, _: &SelectNext, _, cx| this.move_cursor(1, cx)))
            .on_action(cx.listener(|this, _: &LoadSelected, _, cx| this.load_selected(cx)))
            .on_action(cx.listener(|this, _: &PlayPause, _, _| this.play_pause()))
            .on_action(cx.listener(|this, _: &Stop, _, _| this.send(Command::Stop)))
            .on_action(
                cx.listener(|this, _: &TogglePreview, _, _| this.send(Command::TogglePreview)),
            )
            .on_action(cx.listener(|this, _: &Crossfade, _, _| this.send(Command::Crossfade)))
            .on_action(cx.listener(|this, _: &NudgeBack, _, _| this.nudge(-1.0)))
            .on_action(cx.listener(|this, _: &NudgeForward, _, _| this.nudge(1.0)))
            .on_action(cx.listener(|this, _: &NudgeBackFine, _, _| this.nudge(-0.1)))
            .on_action(cx.listener(|this, _: &NudgeForwardFine, _, _| this.nudge(0.1)))
            .on_action(cx.listener(|this, _: &Rescan, _, cx| this.rescan(cx)))
            .on_action(cx.listener(|this, _: &CycleTheme, _, cx| this.cycle_theme(cx)))
            .child(
                h_resizable("columns")
                    .with_state(&self.split)
                    .child(
                        resizable_panel()
                            .size(px(TREE_WIDTH))
                            .size_range(px(TREE_RANGE.0)..px(TREE_RANGE.1))
                            .child(self.render_tree(p, cx)),
                    )
                    .child(resizable_panel().child(self.render_list(p, cx)))
                    .child(
                        resizable_panel()
                            .size(px(DECKS_WIDTH))
                            .size_range(px(DECKS_RANGE.0)..px(DECKS_RANGE.1))
                            .child(self.render_decks(p, cx)),
                    ),
            )
            .when(self.show_settings, |d| d.child(self.render_settings(p, cx)))
    }
}

impl Workspace {
    fn render_tree(&self, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let header = div()
            .flex()
            .items_center()
            .justify_between()
            .px_3()
            .h(px(44.))
            .border_b_1()
            .border_color(p.border)
            .child(caps("LIBRERIA", p.muted))
            .child(
                button("rescan", if self.scanning { "SCAN…" } else { "↻ SCAN" }, p)
                    .h(px(24.))
                    .px_2()
                    .text_size(px(10.5))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.rescan(cx))),
            );
        let rows: Vec<AnyElement> = self
            .tree
            .as_ref()
            .map(|tree| {
                tree.visible_rows(&self.expanded)
                    .into_iter()
                    .enumerate()
                    .map(|(i, (depth, node))| {
                        let selected = match &self.scope {
                            Some(s) => *s == node.path,
                            None => depth == 0,
                        };
                        let open = self.expanded.contains(&node.path);
                        let arrow = if node.children.is_empty() {
                            " "
                        } else if open {
                            "▾"
                        } else {
                            "▸"
                        };
                        let path = node.path.clone();
                        let toggle = node.path.clone();
                        div()
                            .id(("folder", i))
                            .flex()
                            .items_center()
                            .gap_1()
                            .pl(px(10. + depth as f32 * 14.))
                            .pr_2()
                            .h(px(26.))
                            .border_l_2()
                            .border_color(if selected {
                                p.cue
                            } else {
                                gpui::transparent_black()
                            })
                            .cursor_pointer()
                            .hover(|s| s.bg(p.card))
                            .when(selected, |d| {
                                d.bg(p.selected).font_weight(FontWeight::SEMIBOLD)
                            })
                            .child(
                                div()
                                    .id(("arrow", i))
                                    .w(px(12.))
                                    .text_color(p.muted)
                                    .child(arrow)
                                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                        cx.stop_propagation();
                                        this.toggle_folder(toggle.clone(), cx);
                                    })),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .text_ellipsis()
                                    .child(node.name.clone()),
                            )
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                this.select_folder(path.clone(), cx)
                            }))
                            .into_any_element()
                    })
                    .collect()
            })
            .unwrap_or_default();
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(p.panel)
            .child(header)
            .child(
                div()
                    .id("tree")
                    .flex_1()
                    .overflow_y_scroll()
                    .py_1()
                    .children(rows),
            )
            .into_any_element()
    }

    fn render_list(&self, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let crumb: SharedString = match (&self.scope, &self.settings.library_root) {
            (Some(s), Some(r)) => {
                let name = r
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let rel = s.strip_prefix(r).map(|rel| {
                    rel.iter()
                        .map(|c| c.to_string_lossy().into_owned())
                        .collect::<Vec<_>>()
                        .join(" / ")
                });
                format!("{name} / {}", rel.unwrap_or_default())
            }
            (None, Some(r)) => r
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            _ => String::new(),
        }
        .to_uppercase()
        .into();
        let count = self.results.len();
        let body: AnyElement = if self.settings.library_root.is_none() {
            div()
                .flex_1()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap_3()
                .text_color(p.muted)
                .child("Scegli la cartella dove tieni la musica (MP3, FLAC, WAV).")
                .child(
                    filled("pick-empty", "SCEGLI CARTELLA…", p.cue, p)
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.pick_library(cx))),
                )
                .into_any_element()
        } else if count == 0 {
            let msg = if self.scanning {
                "Leggo la libreria…"
            } else if self.search.read(cx).value().is_empty() {
                "Nessun brano in questa cartella."
            } else {
                "Nessun brano trovato."
            };
            div()
                .flex_1()
                .p_4()
                .text_color(p.muted)
                .child(msg)
                .into_any_element()
        } else {
            uniform_list(
                "tracks",
                count,
                cx.processor(move |this, range: Range<usize>, _window, cx| {
                    this.render_rows(range, p, cx)
                }),
            )
            .track_scroll(&self.list_scroll)
            .w_full()
            .flex_1()
            .into_any_element()
        };
        let columns = div()
            .flex()
            .items_center()
            .gap_3()
            .px_3()
            .h(px(26.))
            .bg(p.panel)
            .border_b_1()
            .border_color(p.border)
            .child(div().w(px(14.)))
            .child(caps("TITOLO", p.muted).flex_1())
            .child(caps("ARTISTA · ALBUM", p.muted).flex_1())
            .child(caps("TEMPO", p.muted).w(px(48.)).text_right());
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(p.card)
            .child(
                div()
                    .key_context("Search")
                    .px_3()
                    .pt_2()
                    .pb_1()
                    .child(Input::new(&self.search).text_size(px(15.))),
            )
            .child(
                div()
                    .flex()
                    .justify_between()
                    .px_3()
                    .pb_2()
                    .child(caps(crumb, p.muted))
                    .child(caps(
                        format!("{count} BRANI · DOPPIO CLIC / INVIO → DECK B"),
                        p.muted,
                    )),
            )
            .child(columns)
            .child(body)
            .into_any_element()
    }

    fn render_rows(
        &mut self,
        range: Range<usize>,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let next_id = self.snapshot.next.as_ref().map(|d| d.id);
        let air_id = self
            .snapshot
            .incoming
            .as_ref()
            .or(self.snapshot.on_air.as_ref())
            .map(|d| d.id);
        range
            .filter_map(|row| {
                let t = self.tracks.get(*self.results.get(row)?)?;
                let selected = self.cursor == Some(row);
                let (marker, tint) = if Some(t.id) == air_id {
                    ("A", Some(p.live))
                } else if Some(t.id) == next_id {
                    ("B", Some(p.cue))
                } else {
                    ("", None)
                };
                Some(
                    div()
                        .id(("row", row))
                        .w_full()
                        .h(px(ROW_HEIGHT))
                        .flex()
                        .items_center()
                        .gap_3()
                        .px_3()
                        .border_b_1()
                        .border_color(p.border)
                        .relative()
                        .cursor_pointer()
                        .hover(|s| s.bg(p.panel))
                        .when(selected, |d| {
                            d.bg(p.selected).child(
                                div()
                                    .absolute()
                                    .left_0()
                                    .top_0()
                                    .w(px(3.))
                                    .h_full()
                                    .bg(p.cue),
                            )
                        })
                        .child(
                            div()
                                .w(px(14.))
                                .font_family(DISPLAY_FONT)
                                .font_weight(FontWeight::BOLD)
                                .when_some(tint, |d, c| d.text_color(c))
                                .child(marker),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .font_weight(FontWeight::SEMIBOLD)
                                .when_some(tint, |d, c| d.text_color(c))
                                .child(t.title.clone()),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .text_color(p.muted)
                                .child(match (t.artist.is_empty(), t.album.is_empty()) {
                                    (false, false) => format!("{} · {}", t.artist, t.album),
                                    (false, true) => t.artist.clone(),
                                    _ => t.album.clone(),
                                }),
                        )
                        .child(
                            div()
                                .w(px(48.))
                                .text_right()
                                .font_family(DISPLAY_FONT)
                                .text_size(px(12.))
                                .text_color(p.muted)
                                .child(clock(t.duration_secs)),
                        )
                        .on_click(cx.listener(move |this, ev: &ClickEvent, window, cx| {
                            window.focus(&this.focus, cx);
                            if ev.click_count() >= 2 {
                                this.load_row(row, cx);
                            } else {
                                this.cursor = Some(row);
                                cx.notify();
                            }
                        }))
                        .into_any_element(),
                )
            })
            .collect()
    }

    fn render_decks(&self, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let s = &self.snapshot;
        let alerts: Vec<AnyElement> = self
            .alerts
            .iter()
            .enumerate()
            .map(|(i, a)| {
                div()
                    .id(("alert", i))
                    .flex()
                    .gap_2()
                    .p_2()
                    .rounded(px(RADIUS))
                    .bg(p.warn_bg)
                    .border_l_2()
                    .border_color(p.live)
                    .text_size(px(12.))
                    .cursor_pointer()
                    .child(div().flex_1().min_w_0().child(a.clone()))
                    .child(div().text_color(p.muted).child("✕"))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        if i < this.alerts.len() {
                            this.alerts.remove(i);
                        }
                        cx.notify();
                    }))
                    .into_any_element()
            })
            .collect();

        // During a crossfade, deck A shows the track fading in.
        let air = s.incoming.as_ref().or(s.on_air.as_ref());
        let air_state = if s.incoming.is_some() {
            "MIX"
        } else if s.on_air.is_some() && s.playing {
            "PLAY"
        } else if s.on_air.is_some() {
            "PAUSA"
        } else {
            "—"
        };
        let playing = s.playing && s.on_air.is_some();
        let deck_a = self
            .render_deck(
                Deck::OnAir,
                air,
                ("A", "IN ONDA · CASSE", air_state),
                p.live,
                None,
                p,
                cx,
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .px_3()
                    .child(
                        filled(
                            "playpause",
                            if playing { "❚❚ PAUSA" } else { "▶ PLAY" },
                            p.live,
                            p,
                        )
                        .flex_1()
                        .h(px(38.))
                        .on_click(cx.listener(|this, _: &ClickEvent, _, _| this.play_pause())),
                    )
                    .child(button("stop", "■ STOP", p).h(px(38.)).on_click(
                        cx.listener(|this, _: &ClickEvent, _, _| this.send(Command::Stop)),
                    )),
            )
            .child(div().p_3().child(volume_row(
                &self.master_volume,
                self.settings.master_volume,
                p,
            )));

        let next_state = if s.previewing {
            "CUE ●"
        } else if s.next.is_some() {
            "PRONTO"
        } else {
            "—"
        };
        let deck_b = self
            .render_deck(
                Deck::Next,
                s.next.as_ref(),
                ("B", "PROSSIMO · CUFFIA", next_state),
                p.cue,
                self.preview_pos,
                p,
                cx,
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .px_3()
                    .child(
                        if s.previewing {
                            filled("preview", "◉ CUE ON", p.cue, p)
                        } else {
                            button("preview", "◎ CUE", p)
                        }
                        .flex_1()
                        .h(px(38.))
                        .on_click(cx.listener(
                            |this, _: &ClickEvent, _, _| this.send(Command::TogglePreview),
                        )),
                    )
                    .child(
                        button("nudge-back", "◀ 1S", p)
                            .h(px(38.))
                            .on_click(cx.listener(|this, _: &ClickEvent, _, _| this.nudge(-1.0))),
                    )
                    .child(
                        button("nudge-fwd", "1S ▶", p)
                            .h(px(38.))
                            .on_click(cx.listener(|this, _: &ClickEvent, _, _| this.nudge(1.0))),
                    ),
            )
            .child(
                div()
                    .p_3()
                    .child(volume_row(&self.cue_volume, self.settings.cue_volume, p)),
            );

        let outputs = format!(
            "CASSE: {}  ·  CUFFIA: {}",
            self.settings.master_device.as_deref().unwrap_or("—"),
            self.settings.cue_device.as_deref().unwrap_or("—")
        );
        div()
            .id("decks")
            .size_full()
            .flex()
            .flex_col()
            .gap_3()
            .p_3()
            .bg(p.bg)
            .overflow_y_scroll()
            .children(alerts)
            .child(deck_a)
            .child(self.render_crossfader(p, cx))
            .child(deck_b)
            .child(div().flex_1())
            .child(caps(outputs, p.muted))
            .child(
                button("settings", "⚙ IMPOSTAZIONI", p)
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.open_settings(cx))),
            )
            .into_any_element()
    }

    /// The crossfader: where the mix sits between A and B, and the button
    /// that throws it.
    fn render_crossfader(&self, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let fade = self.snapshot.fade;
        // At rest the fader sits on A; during a crossfade it travels to B.
        let at = fade.unwrap_or(0.0);
        let auto_in = self
            .snapshot
            .on_air
            .as_ref()
            .filter(|_| self.snapshot.next.is_some())
            .map(|on| (on.duration - on.pos - self.settings.crossfade_secs).max(0.0));
        let hint = match (fade, auto_in) {
            (Some(_), _) => "IN CORSO…".to_string(),
            (None, Some(t)) => format!("AUTO TRA {}", clock(t)),
            _ => format!("{:.0} S", self.settings.crossfade_secs),
        };
        let track = div()
            .relative()
            .flex_1()
            .h(px(10.))
            .rounded(px(RADIUS))
            .bg(p.panel)
            .border_1()
            .border_color(p.border)
            .child(
                div()
                    .absolute()
                    .top(px(-6.))
                    .left(gpui::relative(at * 0.94))
                    .w(gpui::relative(0.06))
                    .h(px(20.))
                    .rounded(px(RADIUS))
                    .bg(if fade.is_some() { p.cue } else { p.text }),
            );
        div()
            .flex()
            .flex_col()
            .gap_3()
            .p_3()
            .rounded(px(RADIUS))
            .border_1()
            .border_color(p.border)
            .bg(p.card)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(caps("A", p.live))
                    .child(track)
                    .child(caps("B", p.cue)),
            )
            .child(
                filled(
                    "crossfade",
                    format!("CROSSFADE A → B  ·  {hint}"),
                    p.text,
                    p,
                )
                .h(px(40.))
                .text_size(px(12.5))
                .text_color(p.bg)
                .on_click(cx.listener(|this, _: &ClickEvent, _, _| this.send(Command::Crossfade))),
            )
            .into_any_element()
    }

    #[allow(clippy::too_many_arguments)]
    fn render_deck(
        &self,
        deck: Deck,
        info: Option<&DeckInfo>,
        (letter, title, state): (&'static str, &'static str, &'static str),
        color: Hsla,
        preview: Option<f32>,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> Div {
        let header = div()
            .flex()
            .items_center()
            .justify_between()
            .px_3()
            .h(px(26.))
            .bg(color)
            .text_color(p.on_accent)
            .font_family(DISPLAY_FONT)
            .font_weight(FontWeight::BOLD)
            .text_size(px(11.))
            .child(format!("DECK {letter} · {title}"))
            .child(state);
        let card = div()
            .flex()
            .flex_col()
            .rounded(px(RADIUS))
            .border_1()
            .border_color(color)
            .bg(p.card)
            .child(header);
        let body = div().flex().flex_col().gap_3().p_3();
        let Some(info) = info else {
            let empty = match deck {
                Deck::OnAir => "Niente in onda.",
                Deck::Next => "Doppio clic su un brano per caricarlo qui.",
            };
            return card.child(body.child(div().text_color(p.muted).py_4().child(empty)));
        };
        let track = self.track(info.id);
        let title = track.map_or_else(|| "?".to_string(), |t| t.title.clone());
        let artist = track.map(|t| t.artist.clone()).unwrap_or_default();
        let dur = info.duration.max(0.001);
        let (big, small) = match deck {
            Deck::OnAir => (
                format!("-{}", clock(info.duration - info.pos)),
                format!("{} / {}", clock(info.pos), clock(info.duration)),
            ),
            Deck::Next => (
                format!("▸{}", clock(info.start)),
                preview.map_or_else(
                    || format!("DURATA {}", clock(info.duration)),
                    |pos| format!("CUE {}", clock(pos)),
                ),
            ),
        };
        let readout = div()
            .flex()
            .items_end()
            .justify_between()
            .gap_2()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .min_w_0()
                    .flex_1()
                    .child(
                        div()
                            .text_size(px(16.))
                            .font_weight(FontWeight::BOLD)
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .child(title),
                    )
                    .child(div().text_color(p.muted).text_size(px(12.)).child(artist)),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_end()
                    .child(
                        div()
                            .font_family(DISPLAY_FONT)
                            .font_weight(FontWeight::BOLD)
                            .text_size(px(26.))
                            .line_height(px(28.))
                            .text_color(color)
                            .child(big),
                    )
                    .child(caps(small, p.muted)),
            );

        let peaks = self
            .waves
            .get(&info.id)
            .cloned()
            .unwrap_or_else(|| Arc::new(vec![0.05; WAVE_BARS]));
        let (head, marker) = match deck {
            Deck::OnAir => (Some(info.pos / dur), None),
            Deck::Next => (preview.map(|pos| pos / dur), Some(info.start / dur)),
        };
        let bar_of = |f: f32| ((f * WAVE_BARS as f32) as usize).min(WAVE_BARS - 1);
        let head_bar = head.map(bar_of);
        let marker_bar = marker.map(bar_of);
        let (lit, dim) = match deck {
            Deck::OnAir => (p.live, p.live_dim),
            Deck::Next => (p.cue, p.cue_dim),
        };
        let bars: Vec<AnyElement> = peaks
            .iter()
            .enumerate()
            .map(|(i, &peak)| {
                // Played part bright, the rest dim; on deck B what comes
                // before the start point is skipped, so it stays grey.
                let fill = match deck {
                    Deck::OnAir if head_bar.is_some_and(|h| i <= h) => lit,
                    Deck::OnAir => dim,
                    Deck::Next if marker_bar.is_some_and(|m| i < m) => p.wave,
                    Deck::Next if head_bar.is_some_and(|h| i <= h) => lit,
                    Deck::Next => dim,
                };
                let h = (peak.clamp(0.0, 1.0) * WAVE_HEIGHT).max(2.);
                let id = if deck == Deck::OnAir {
                    "air-bar"
                } else {
                    "next-bar"
                };
                div()
                    .id((id, i))
                    .relative()
                    .flex_1()
                    .h_full()
                    .flex()
                    .items_center()
                    .cursor_pointer()
                    .child(div().w_full().h(px(h)).bg(fill))
                    .when(head_bar == Some(i), |d| {
                        d.child(div().absolute().left_0().w(px(2.)).h_full().bg(p.text))
                    })
                    .when(marker_bar == Some(i), |d| {
                        d.child(div().absolute().left_0().w(px(3.)).h_full().bg(color))
                    })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, _| this.click_wave(deck, i)),
                    )
                    .into_any_element()
            })
            .collect();
        let wave = div()
            .h(px(WAVE_HEIGHT + 8.))
            .px_1()
            .flex()
            .gap(px(1.))
            .bg(p.bg)
            .rounded(px(RADIUS))
            .children(bars);
        let help = match deck {
            Deck::OnAir => "CLIC SULL'ONDA = SALTA",
            Deck::Next => "CLIC SULL'ONDA = PUNTO DI PARTENZA · ⌥← ⌥→",
        };
        card.child(body.child(readout).child(wave).child(caps(help, p.muted)))
    }

    fn render_settings(&self, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let device_list = |role: Role, cx: &mut Context<Self>| -> AnyElement {
            let current = match role {
                Role::Master => self.settings.master_device.clone(),
                Role::Cue => self.settings.cue_device.clone(),
            };
            let tag = if role == Role::Master { "m" } else { "c" };
            let mut rows: Vec<AnyElement> = self
                .devices
                .iter()
                .enumerate()
                .map(|(i, name)| {
                    let on = current.as_deref() == Some(name.as_str());
                    let n = name.clone();
                    div()
                        .id(SharedString::from(format!("dev-{tag}-{i}")))
                        .px_2()
                        .py_1()
                        .rounded(px(RADIUS))
                        .cursor_pointer()
                        .hover(|s| s.bg(p.panel))
                        .when(on, |d| d.bg(p.selected).font_weight(FontWeight::SEMIBOLD))
                        .child(format!("{} {name}", if on { "◉" } else { "○" }))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.set_device(role, n.clone(), cx)
                        }))
                        .into_any_element()
                })
                .collect();
            if let Some(cur) = &current
                && !self.devices.contains(cur)
            {
                rows.push(
                    div()
                        .px_2()
                        .text_color(p.live)
                        .child(format!("◉ {cur} (non collegato)"))
                        .into_any_element(),
                );
            }
            div()
                .flex()
                .flex_col()
                .gap(px(2.))
                .children(rows)
                .into_any_element()
        };
        let section = |title: &'static str,
                       test_id: &'static str,
                       role: Role,
                       color: Hsla,
                       cx: &mut Context<Self>| {
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .flex()
                        .justify_between()
                        .items_center()
                        .child(caps(title, color))
                        .child(
                            button(test_id, "▶ PROVA", p)
                                .h(px(24.))
                                .on_click(cx.listener(move |this, _: &ClickEvent, _, _| {
                                    this.send(Command::TestTone(role))
                                })),
                        ),
                )
                .child(device_list(role, cx))
        };
        let same = self.settings.master_device.is_some()
            && self.settings.master_device == self.settings.cue_device;
        let root = self.settings.library_root.as_ref().map_or_else(
            || "Nessuna cartella scelta".to_string(),
            |r| r.display().to_string(),
        );

        let themes: Vec<AnyElement> = Theme::all()
            .into_iter()
            .enumerate()
            .map(|(i, t)| {
                let tp = t.palette();
                let on = t.name == self.theme.name;
                div()
                    .id(("theme", i))
                    .w(px(124.))
                    .flex()
                    .flex_col()
                    .gap_1()
                    .p_1()
                    .rounded(px(RADIUS))
                    .border_2()
                    .border_color(if on { p.text } else { p.border })
                    .cursor_pointer()
                    .hover(|s| s.border_color(p.muted))
                    .child(
                        div()
                            .h(px(38.))
                            .flex()
                            .items_end()
                            .gap(px(3.))
                            .p_1()
                            .bg(tp.bg)
                            .children([0.5, 0.8, 1.0, 0.6, 0.9, 0.4].iter().enumerate().map(
                                |(j, h)| {
                                    div().flex_1().h(px(28. * h)).bg(if j < 3 {
                                        tp.live
                                    } else {
                                        tp.cue
                                    })
                                },
                            )),
                    )
                    .child(
                        div()
                            .text_size(px(11.))
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .child(t.name),
                    )
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.set_theme(t, cx)))
                    .into_any_element()
            })
            .collect();

        div()
            .id("settings-backdrop")
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(gpui::hsla(0., 0., 0., 0.55))
            .on_click(cx.listener(|_, _: &ClickEvent, _, cx| cx.stop_propagation()))
            .child(
                div()
                    .id("settings")
                    .w(px(640.))
                    .h(px(560.))
                    .flex()
                    .flex_col()
                    .rounded(px(RADIUS))
                    .bg(p.card)
                    .border_1()
                    .border_color(p.border)
                    .child(
                        div()
                            .flex()
                            .items_end()
                            .gap_4()
                            .px_5()
                            .pt_4()
                            .border_b_1()
                            .border_color(p.border)
                            .child(
                                div()
                                    .pb_3()
                                    .font_family(DISPLAY_FONT)
                                    .text_size(px(16.))
                                    .font_weight(FontWeight::BOLD)
                                    .child("IMPOSTAZIONI"),
                            )
                            .child(div().flex().children(SettingsTab::ALL.into_iter().map(|tab| {
                                let on = tab == self.settings_tab;
                                div()
                                    .id(tab.label())
                                    .px_3()
                                    .pb_2()
                                    .pt_1()
                                    .border_b_2()
                                    .border_color(if on { p.cue } else { gpui::transparent_black() })
                                    .font_family(DISPLAY_FONT)
                                    .text_size(px(11.5))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(if on { p.text } else { p.muted })
                                    .cursor_pointer()
                                    .hover(|s| s.text_color(p.text))
                                    .child(tab.label())
                                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.show_tab(tab, cx)))
                            }))),
                    )
                    .child(
                        div()
                            .id("settings-body")
                            .flex_1()
                            .overflow_y_scroll()
                            .flex()
                            .flex_col()
                            .gap_5()
                            .p_5()
                            .child(match self.settings_tab {
                                SettingsTab::Library => div()
                                    .flex()
                                    .flex_col()
                                    .gap_2()
                                    .child(caps("CARTELLA DELLA MUSICA", p.muted))
                                    .child(div().child(root))
                                    .child(
                                        div()
                                            .flex()
                                            .gap_2()
                                            .child(button("pick", "SCEGLI CARTELLA…", p).on_click(
                                                cx.listener(|this, _: &ClickEvent, _, cx| this.pick_library(cx)),
                                            ))
                                            .child(button("rescan-settings", "↻ AGGIORNA LIBRERIA", p).on_click(
                                                cx.listener(|this, _: &ClickEvent, _, cx| this.rescan(cx)),
                                            )),
                                    )
                                    .child(div().text_color(p.muted).text_size(px(12.)).child(format!(
                                        "{} brani nell'indice. Formati letti: MP3, FLAC, WAV.",
                                        self.tracks.len()
                                    )))
                                    .into_any_element(),
                                SettingsTab::Audio => div()
                                    .flex()
                                    .flex_col()
                                    .gap_5()
                                    .child(section("DECK A · CASSE (MASTER)", "test-master", Role::Master, p.live, cx))
                                    .child(section("DECK B · CUFFIA (CUE)", "test-cue", Role::Cue, p.cue, cx))
                                    .when(same, |d| {
                                        d.child(div().p_2().rounded(px(RADIUS)).bg(p.warn_bg).child(
                                            "Casse e cuffia sono lo stesso dispositivo: l'anteprima si sentirebbe anche dalle casse.",
                                        ))
                                    })
                                    .child(div().flex().child(button("refresh-devices", "↻ AGGIORNA DISPOSITIVI", p).on_click(
                                        cx.listener(|this, _: &ClickEvent, _, cx| this.show_tab(SettingsTab::Audio, cx)),
                                    )))
                                    .into_any_element(),
                                SettingsTab::Mix => div()
                                    .flex()
                                    .flex_col()
                                    .gap_3()
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap_2()
                                            .child(caps("DURATA DEL CROSSFADE", p.muted).flex_1())
                                            .child(button("fade-down", "−", p).on_click(cx.listener(
                                                |this, _: &ClickEvent, _, cx| this.change_crossfade(-0.5, cx),
                                            )))
                                            .child(
                                                div()
                                                    .w(px(64.))
                                                    .text_center()
                                                    .font_family(DISPLAY_FONT)
                                                    .child(format!("{:.1} s", self.settings.crossfade_secs)),
                                            )
                                            .child(button("fade-up", "+", p).on_click(cx.listener(
                                                |this, _: &ClickEvent, _, cx| this.change_crossfade(0.5, cx),
                                            ))),
                                    )
                                    .child(div().text_color(p.muted).text_size(px(12.)).child(
                                        "Vale per il pulsante Crossfade e per il passaggio automatico, che parte da solo in modo da finire insieme al brano in onda.",
                                    ))
                                    .into_any_element(),
                                SettingsTab::Look => div()
                                    .flex()
                                    .flex_col()
                                    .gap_2()
                                    .child(caps("TEMA  ·  ⌘T PER CAMBIARE AL VOLO", p.muted))
                                    .child(div().flex().flex_wrap().gap_2().children(themes))
                                    .into_any_element(),
                                SettingsTab::Keys => div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .children(KEYS.iter().map(|(k, what)| {
                                        div()
                                            .flex()
                                            .gap_3()
                                            .py_1()
                                            .border_b_1()
                                            .border_color(p.border)
                                            .child(div().w(px(150.)).font_family(DISPLAY_FONT).text_color(p.cue).child(*k))
                                            .child(div().child(*what))
                                    }))
                                    .into_any_element(),
                            }),
                    )
                    .child(
                        div()
                            .flex()
                            .justify_end()
                            .px_5()
                            .py_3()
                            .border_t_1()
                            .border_color(p.border)
                            .child(filled("close-settings", "FATTO", p.cue, p).on_click(
                                cx.listener(|this, _: &ClickEvent, window, cx| this.close_settings(window, cx)),
                            )),
                    ),
            )
            .into_any_element()
    }
}
