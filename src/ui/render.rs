use std::ops::Range;
use std::sync::Arc;

use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, ClickEvent, Context, ElementId, FontWeight, Hsla, InteractiveElement, IntoElement,
    MouseButton, ParentElement, Render, SharedString, StatefulInteractiveElement, Styled, Window,
    div, px, uniform_list,
};
use gpui_component::input::Input;

use super::theme::{Palette, palette};
use super::{Deck, WAVE_BARS, Workspace};
use crate::actions::*;
use crate::audio::engine::{Command, DeckInfo, Role};

const TREE_WIDTH: f32 = 230.;
const DECKS_WIDTH: f32 = 360.;
const ROW_HEIGHT: f32 = 30.;

fn clock(secs: f32) -> String {
    let s = secs.max(0.0) as u32;
    format!("{}:{:02}", s / 60, s % 60)
}

impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(window.appearance());
        div()
            .id("workspace")
            .key_context(WORKSPACE)
            .track_focus(&self.focus)
            .relative()
            .size_full()
            .flex()
            .bg(p.bg)
            .text_color(p.text)
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
            .on_action(cx.listener(|this, _: &TogglePreview, _, _| this.send(Command::TogglePreview)))
            .on_action(cx.listener(|this, _: &Crossfade, _, _| this.send(Command::Crossfade)))
            .on_action(cx.listener(|this, _: &NudgeBack, _, _| this.nudge(-1.0)))
            .on_action(cx.listener(|this, _: &NudgeForward, _, _| this.nudge(1.0)))
            .on_action(cx.listener(|this, _: &NudgeBackFine, _, _| this.nudge(-0.1)))
            .on_action(cx.listener(|this, _: &NudgeForwardFine, _, _| this.nudge(0.1)))
            .on_action(cx.listener(|this, _: &Rescan, _, cx| this.rescan(cx)))
            .child(self.render_tree(p, cx))
            .child(self.render_list(p, cx))
            .child(self.render_decks(p, cx))
            .when(self.show_settings, |d| d.child(self.render_settings(p, cx)))
    }
}

fn button(id: impl Into<ElementId>, label: impl Into<SharedString>, p: Palette) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .px_3()
        .py_1()
        .rounded_md()
        .border_1()
        .border_color(p.border)
        .bg(p.card)
        .cursor_pointer()
        .hover(|s| s.bg(p.selected))
        .child(label.into())
}

fn primary(id: impl Into<ElementId>, label: impl Into<SharedString>, color: Hsla, p: Palette) -> gpui::Stateful<gpui::Div> {
    button(id, label, p).bg(color).border_color(color).text_color(p.on_accent).font_weight(FontWeight::SEMIBOLD)
}

fn label(text: impl Into<SharedString>, color: Hsla) -> gpui::Div {
    div().text_size(px(10.5)).font_weight(FontWeight::BOLD).text_color(color).child(text.into())
}

impl Workspace {
    fn render_tree(&self, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let mut col = div()
            .w(px(TREE_WIDTH))
            .h_full()
            .flex()
            .flex_col()
            .flex_none()
            .bg(p.panel)
            .border_r_1()
            .border_color(p.border);
        col = col.child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .px_3()
                .py_2()
                .child(label("LIBRERIA", p.muted))
                .child(
                    button("rescan", if self.scanning { "Aggiorno…" } else { "Aggiorna" }, p)
                        .text_size(px(11.))
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.rescan(cx))),
                ),
        );
        let Some(tree) = &self.tree else {
            return col.into_any_element();
        };
        let rows: Vec<AnyElement> = tree
            .visible_rows(&self.expanded)
            .into_iter()
            .enumerate()
            .map(|(i, (depth, node))| {
                let is_root = depth == 0;
                let selected = match &self.scope {
                    Some(s) => *s == node.path,
                    None => is_root,
                };
                let open = self.expanded.contains(&node.path);
                let arrow = if node.children.is_empty() { " " } else if open { "▾" } else { "▸" };
                let path = node.path.clone();
                let toggle_path = node.path.clone();
                div()
                    .id(("folder", i))
                    .flex()
                    .items_center()
                    .gap_1()
                    .pl(px(8. + depth as f32 * 14.))
                    .pr_2()
                    .py(px(3.))
                    .mx_1()
                    .rounded_sm()
                    .cursor_pointer()
                    .when(selected, |d| d.bg(p.selected).font_weight(FontWeight::SEMIBOLD))
                    .child(
                        div()
                            .id(("arrow", i))
                            .w(px(12.))
                            .text_color(p.muted)
                            .child(arrow)
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                cx.stop_propagation();
                                this.toggle_folder(toggle_path.clone(), cx);
                            })),
                    )
                    .child(div().flex_1().overflow_hidden().whitespace_nowrap().text_ellipsis().child(node.name.clone()))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.select_folder(path.clone(), cx)))
                    .into_any_element()
            })
            .collect();
        col.child(div().id("tree").flex_1().overflow_y_scroll().pb_2().children(rows)).into_any_element()
    }

    fn render_list(&self, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let crumb: SharedString = match (&self.scope, &self.settings.library_root) {
            (Some(s), Some(r)) => s.strip_prefix(r).map_or_else(|_| s.display().to_string(), |rel| {
                let name = r.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                format!("{name} › {}", rel.iter().map(|c| c.to_string_lossy()).collect::<Vec<_>>().join(" › "))
            }),
            (None, Some(r)) => r.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
            _ => String::new(),
        }
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
                    primary("pick-empty", "Scegli cartella…", p.cue, p)
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
            div().flex_1().p_4().text_color(p.muted).child(msg).into_any_element()
        } else {
            uniform_list(
                "tracks",
                count,
                cx.processor(move |this, range: Range<usize>, _window, cx| this.render_rows(range, p, cx)),
            )
            .track_scroll(&self.list_scroll)
            .w_full()
            .flex_1()
            .into_any_element()
        };
        div()
            .flex_1()
            .min_w(px(320.))
            .h_full()
            .flex()
            .flex_col()
            .bg(p.card)
            .child(
                div()
                    .key_context("Search")
                    .p_3()
                    .pb_1()
                    .child(Input::new(&self.search).text_size(px(14.))),
            )
            .child(
                div()
                    .flex()
                    .justify_between()
                    .px_3()
                    .pb_2()
                    .text_size(px(11.5))
                    .text_color(p.muted)
                    .child(crumb)
                    .child(format!("{count} brani · doppio clic o Invio → Prossimo")),
            )
            .child(div().h(px(1.)).bg(p.border))
            .child(body)
            .into_any_element()
    }

    fn render_rows(&mut self, range: Range<usize>, p: Palette, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let next_id = self.snapshot.next.as_ref().map(|d| d.id);
        let air_id = self.snapshot.on_air.as_ref().map(|d| d.id);
        range
            .filter_map(|row| {
                let t = self.tracks.get(*self.results.get(row)?)?;
                let selected = self.cursor == Some(row);
                let marker = if Some(t.id) == air_id {
                    Some(("●", p.live))
                } else if Some(t.id) == next_id {
                    Some(("◐", p.cue))
                } else {
                    None
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
                        .cursor_pointer()
                        .when(selected, |d| d.bg(p.selected))
                        .child(div().w(px(12.)).when_some(marker, |d, (m, c)| d.text_color(c).child(m)))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .font_weight(FontWeight::SEMIBOLD)
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
                        .child(div().w(px(44.)).text_right().text_color(p.muted).child(clock(t.duration_secs)))
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
                    .rounded_md()
                    .bg(p.warn_bg)
                    .text_size(px(12.))
                    .child(div().flex_1().child(a.clone()))
                    .child(div().text_color(p.muted).cursor_pointer().child("✕"))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        if i < this.alerts.len() {
                            this.alerts.remove(i);
                        }
                        cx.notify();
                    }))
                    .into_any_element()
            })
            .collect();

        // During a crossfade the incoming track is shown on air, fading in.
        let air = s.incoming.as_ref().or(s.on_air.as_ref());
        let air_label = if s.incoming.is_some() {
            "● IN ONDA · CASSE · crossfade…"
        } else if s.on_air.is_some() && !s.playing {
            "● IN ONDA · CASSE · in pausa"
        } else {
            "● IN ONDA · CASSE"
        };
        let on_air = self.render_deck(Deck::OnAir, air, air_label, p.live, None, p, cx).child(
            div()
                .flex()
                .gap_2()
                .child(
                    button("playpause", if s.playing && s.on_air.is_some() { "Pausa" } else { "Play" }, p)
                        .on_click(cx.listener(|this, _: &ClickEvent, _, _| this.play_pause())),
                )
                .child(button("stop", "Stop", p).on_click(cx.listener(|this, _: &ClickEvent, _, _| this.send(Command::Stop)))),
        );

        let fade = div().flex().justify_center().child(
            primary("crossfade", format!("↑ Crossfade {:.0} s", self.settings.crossfade_secs), p.cue, p)
                .on_click(cx.listener(|this, _: &ClickEvent, _, _| this.send(Command::Crossfade))),
        );

        let next_label = if s.previewing { "◐ PROSSIMO · CUFFIA · in ascolto" } else { "◐ PROSSIMO · CUFFIA" };
        let next = self
            .render_deck(Deck::Next, s.next.as_ref(), next_label, p.cue, self.preview_pos, p, cx)
            .child(
                div()
                    .flex()
                    .gap_2()
                    .items_center()
                    .child(
                        button("preview", if s.previewing { "■ Ferma anteprima" } else { "▶ Anteprima" }, p)
                            .on_click(cx.listener(|this, _: &ClickEvent, _, _| this.send(Command::TogglePreview))),
                    )
                    .child(div().text_size(px(11.)).text_color(p.muted).child("clic sull'onda = punto di partenza")),
            );

        let outputs = format!(
            "Casse: {} · Cuffia: {}",
            self.settings.master_device.as_deref().unwrap_or("—"),
            self.settings.cue_device.as_deref().unwrap_or("—")
        );
        div()
            .w(px(DECKS_WIDTH))
            .flex_none()
            .h_full()
            .flex()
            .flex_col()
            .gap_3()
            .p_3()
            .bg(p.panel)
            .border_l_1()
            .border_color(p.border)
            .children(alerts)
            .child(on_air)
            .child(fade)
            .child(next)
            .child(div().flex_1())
            .child(div().text_size(px(11.)).text_color(p.muted).child(outputs))
            .child(
                button("settings", "Impostazioni…", p)
                    .text_size(px(12.))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.open_settings(cx))),
            )
            .into_any_element()
    }

    #[allow(clippy::too_many_arguments)]
    fn render_deck(
        &self,
        deck: Deck,
        info: Option<&DeckInfo>,
        title: &'static str,
        color: Hsla,
        preview: Option<f32>,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let mut card = div()
            .flex()
            .flex_col()
            .gap_2()
            .p_3()
            .rounded_lg()
            .border_1()
            .border_color(p.border)
            .bg(p.card)
            .child(label(title, color));
        let Some(info) = info else {
            let empty = match deck {
                Deck::OnAir => "Niente in onda.",
                Deck::Next => "Doppio clic su un brano per caricarlo qui.",
            };
            return card.child(div().text_color(p.muted).py_3().child(empty));
        };
        let track = self.track(info.id);
        let title = track.map_or_else(|| "?".to_string(), |t| t.title.clone());
        let artist = track.map(|t| t.artist.clone()).unwrap_or_default();
        let peaks = self.waves.get(&info.id).cloned().unwrap_or_else(|| Arc::new(vec![0.05; WAVE_BARS]));
        let dur = info.duration.max(0.001);
        let (played_to, marker) = match deck {
            Deck::OnAir => (info.pos / dur, None),
            Deck::Next => (preview.map_or(0.0, |pos| pos / dur), Some(info.start / dur)),
        };
        let marker_bar = marker.map(|m| ((m * WAVE_BARS as f32) as usize).min(WAVE_BARS - 1));
        let bars: Vec<AnyElement> = peaks
            .iter()
            .enumerate()
            .map(|(i, &peak)| {
                let frac = i as f32 / WAVE_BARS as f32;
                let lit = frac < played_to;
                let h = (peak.clamp(0.0, 1.0) * 40.).max(2.);
                let id_base = if deck == Deck::OnAir { "air-bar" } else { "next-bar" };
                div()
                    .id((id_base, i))
                    .flex_1()
                    .h_full()
                    .flex()
                    .items_center()
                    .cursor_pointer()
                    .child(div().w_full().h(px(h)).bg(if lit { color } else { p.wave }))
                    .when(marker_bar == Some(i), |d| d.child(div().absolute().w(px(2.)).h_full().bg(color)))
                    .relative()
                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, _| this.click_wave(deck, i)))
                    .into_any_element()
            })
            .collect();
        let right = match deck {
            Deck::OnAir => format!("{} / {}", clock(info.pos), clock(info.duration)),
            Deck::Next => format!("parte da {}", clock(info.start)),
        };
        card = card
            .child(
                div()
                    .flex()
                    .flex_col()
                    .child(div().font_weight(FontWeight::SEMIBOLD).overflow_hidden().whitespace_nowrap().text_ellipsis().child(title))
                    .child(div().text_color(p.muted).text_size(px(12.)).child(artist)),
            )
            .child(div().h(px(44.)).flex().gap(px(1.)).children(bars))
            .child(div().flex().justify_end().text_size(px(11.5)).text_color(p.muted).child(right));
        card
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
                        .rounded_sm()
                        .cursor_pointer()
                        .when(on, |d| d.bg(p.selected).font_weight(FontWeight::SEMIBOLD))
                        .child(format!("{} {name}", if on { "◉" } else { "○" }))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.set_device(role, n.clone(), cx)))
                        .into_any_element()
                })
                .collect();
            if let Some(cur) = &current
                && !self.devices.contains(cur)
            {
                rows.push(div().px_2().text_color(p.live).child(format!("◉ {cur} (non collegato)")).into_any_element());
            }
            div().flex().flex_col().gap(px(2.)).children(rows).into_any_element()
        };
        let section = |title: &'static str, test_id: &'static str, role: Role, cx: &mut Context<Self>| {
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .flex()
                        .justify_between()
                        .items_center()
                        .child(div().font_weight(FontWeight::SEMIBOLD).child(title))
                        .child(
                            button(test_id, "Prova", p)
                                .text_size(px(11.))
                                .on_click(cx.listener(move |this, _: &ClickEvent, _, _| this.send(Command::TestTone(role)))),
                        ),
                )
                .child(device_list(role, cx))
        };
        let same = self.settings.master_device.is_some() && self.settings.master_device == self.settings.cue_device;
        let root = self
            .settings
            .library_root
            .as_ref()
            .map_or_else(|| "Nessuna cartella scelta".to_string(), |r| r.display().to_string());

        div()
            .id("settings-backdrop")
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(gpui::hsla(0., 0., 0., 0.35))
            .on_click(cx.listener(|_, _: &ClickEvent, _, cx| cx.stop_propagation()))
            .child(
                div()
                    .id("settings")
                    .w(px(520.))
                    .max_h(px(640.))
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .p_5()
                    .rounded_lg()
                    .bg(p.card)
                    .border_1()
                    .border_color(p.border)
                    .child(div().text_size(px(18.)).font_weight(FontWeight::BOLD).child("Impostazioni"))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(div().font_weight(FontWeight::SEMIBOLD).child("Cartella della musica"))
                            .child(div().text_color(p.muted).child(root))
                            .child(
                                div().flex().child(
                                    button("pick", "Scegli cartella…", p)
                                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.pick_library(cx))),
                                ),
                            ),
                    )
                    .child(section("Casse (Master)", "test-master", Role::Master, cx))
                    .child(section("Cuffia (Cue · anteprima)", "test-cue", Role::Cue, cx))
                    .when(same, |d| {
                        d.child(
                            div()
                                .p_2()
                                .rounded_md()
                                .bg(p.warn_bg)
                                .child("Casse e cuffia sono lo stesso dispositivo: l'anteprima si sentirebbe anche dalle casse."),
                        )
                    })
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(div().flex_1().font_weight(FontWeight::SEMIBOLD).child("Durata del crossfade"))
                            .child(button("fade-down", "−", p).on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.change_crossfade(-0.5, cx))))
                            .child(div().w(px(56.)).text_center().child(format!("{:.1} s", self.settings.crossfade_secs)))
                            .child(button("fade-up", "+", p).on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.change_crossfade(0.5, cx)))),
                    )
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .child(
                                button("refresh-devices", "Aggiorna dispositivi", p)
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.open_settings(cx))),
                            )
                            .child(
                                primary("close-settings", "Fatto", p.cue, p)
                                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.close_settings(window, cx))),
                            ),
                    ),
            )
            .into_any_element()
    }
}
