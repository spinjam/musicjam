//! The setlist panel under the track list, and the dialogs that open,
//! name and save setlists.

use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, ClickEvent, Context, FontWeight, InteractiveElement, IntoElement, MouseButton,
    ParentElement, SharedString, StatefulInteractiveElement, Styled, div, px,
};
use gpui_component::input::Input;
use gpui_component::menu::{ContextMenuExt, PopupMenuItem};

use super::render::{ROW_HEIGHT, button, caps, clock, filled};
use super::theme::{DISPLAY_FONT, Palette, RADIUS};
use super::{Dialog, Then, Workspace};
use crate::actions::*;

impl Workspace {
    pub(super) fn render_setlist(&self, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let pl = &self.playlist;
        let name = pl.name.clone().unwrap_or_else(|| "Senza nome".into());
        let title = format!(
            "SCALETTA · {}{}",
            name.to_uppercase(),
            if pl.dirty { " ●" } else { "" }
        );
        let summary = format!(
            "{} BRANI · {}",
            pl.entries.len(),
            clock(self.setlist_length())
        );
        let header = div()
            .flex()
            .flex_col()
            .gap_2()
            .px_3()
            .py_2()
            .bg(p.panel)
            .border_t_1()
            .border_b_1()
            .border_color(p.border)
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(caps(title, if pl.dirty { p.live } else { p.text }))
                    .child(caps(summary, p.muted)),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(small_button("sl-open", "APRI…", p).on_click(cx.listener(
                        |this, _: &ClickEvent, window, cx| {
                            this.show_dialog(Dialog::Open, window, cx)
                        },
                    )))
                    .child(small_button("sl-new", "NUOVA", p).on_click(cx.listener(
                        |this, _: &ClickEvent, window, cx| {
                            this.replace_setlist(Then::New, window, cx)
                        },
                    )))
                    .child(
                        if pl.dirty {
                            filled("sl-save", "SALVA", p.live, p)
                                .h(px(24.))
                                .px_2()
                                .text_size(px(10.5))
                        } else {
                            small_button("sl-save", "SALVA", p)
                        }
                        .on_click(cx.listener(
                            |this, _: &ClickEvent, window, cx| this.save_setlist(None, window, cx),
                        )),
                    )
                    .child(
                        small_button("sl-save-as", "SALVA COME…", p).on_click(cx.listener(
                            |this, _: &ClickEvent, window, cx| {
                                this.show_dialog(
                                    Dialog::Name {
                                        rename: None,
                                        then: None,
                                    },
                                    window,
                                    cx,
                                )
                            },
                        )),
                    )
                    .when(pl.dirty && pl.id.is_some(), |d| {
                        d.child(small_button("sl-revert", "RIPRISTINA", p).on_click(
                            cx.listener(|this, _: &ClickEvent, _, cx| this.revert_setlist(cx)),
                        ))
                    }),
            );
        let columns = div()
            .flex()
            .items_center()
            .gap_3()
            .px_3()
            .h(px(24.))
            .border_b_1()
            .border_color(p.border)
            .child(caps("#", p.muted).w(px(22.)))
            .child(caps("", p.muted).w(px(14.)))
            .child(caps("TITOLO", p.muted).flex_1())
            .child(caps("ARTISTA", p.muted).flex_1())
            .child(caps("INIZIO", p.muted).w(px(44.)).text_right())
            .child(caps("FINE", p.muted).w(px(44.)).text_right())
            .child(caps("DURATA", p.muted).w(px(48.)).text_right());

        let rows: Vec<AnyElement> =
            pl.entries
                .iter()
                .enumerate()
                .filter_map(|(i, e)| {
                    let v = self.entry_view(e.uid)?;
                    let uid = e.uid;
                    let (mark, tint) = if v.missing {
                        ("!", Some(p.muted))
                    } else if pl.on_air == Some(uid) {
                        ("A", Some(p.live))
                    } else if pl.next == Some(uid) {
                        ("B", Some(p.cue))
                    } else if pl.played.contains(&uid) {
                        ("✓", Some(p.muted))
                    } else {
                        ("", None)
                    };
                    let selected = self.setlist_cursor == Some(uid);
                    let point = |secs: f32, manual: bool| {
                        div()
                            .w(px(44.))
                            .text_right()
                            .font_family(DISPLAY_FONT)
                            .text_size(px(12.))
                            .when(manual, |d| d.font_weight(FontWeight::BOLD))
                            .when(!manual, |d| d.text_color(p.muted))
                            .child(clock(secs))
                    };
                    Some(
                        div()
                            .id(("sl-row", uid as usize))
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
                            .when(v.missing || pl.played.contains(&uid), |d| d.opacity(0.55))
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
                                    .w(px(22.))
                                    .font_family(DISPLAY_FONT)
                                    .text_size(px(12.))
                                    .text_color(p.muted)
                                    .child(format!("{}", i + 1)),
                            )
                            .child(
                                div()
                                    .w(px(14.))
                                    .font_family(DISPLAY_FONT)
                                    .font_weight(FontWeight::BOLD)
                                    .when_some(tint, |d, c| d.text_color(c))
                                    .child(mark),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .text_ellipsis()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .when_some(tint.filter(|_| !v.missing), |d, c| d.text_color(c))
                                    .when(v.missing, |d| d.line_through())
                                    .child(v.title.clone()),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .text_ellipsis()
                                    .text_color(p.muted)
                                    .child(v.artist.clone()),
                            )
                            .child(point(v.start, v.manual_start))
                            .child(point(v.end, v.manual_end))
                            .child(
                                div()
                                    .w(px(48.))
                                    .text_right()
                                    .font_family(DISPLAY_FONT)
                                    .text_size(px(12.))
                                    .text_color(p.muted)
                                    .child(clock(v.end - v.start)),
                            )
                            .on_click(cx.listener(move |this, ev: &ClickEvent, window, cx| {
                                this.setlist_select(uid, window, cx);
                                if ev.click_count() >= 2 {
                                    this.setlist_load(uid, cx);
                                }
                            }))
                            .on_mouse_down(
                                MouseButton::Right,
                                cx.listener(move |this, _, window, cx| {
                                    this.setlist_select(uid, window, cx)
                                }),
                            )
                            .context_menu({
                                let ws = cx.entity().downgrade();
                                move |menu, _, _| {
                                    let item = |label: &'static str,
                                                f: fn(
                                        &mut Workspace,
                                        u64,
                                        &mut Context<Workspace>,
                                    )| {
                                        let ws = ws.clone();
                                        PopupMenuItem::new(label).on_click(move |_, _, cx| {
                                            let _ = ws.update(cx, |this, cx| f(this, uid, cx));
                                        })
                                    };
                                    menu.item(item("Carica nel Deck B (prossimo)", |w, u, cx| {
                                        w.setlist_load(u, cx)
                                    }))
                                    .item(item("Carica nel Deck A (casse)", |w, u, cx| {
                                        w.setlist_load_on_air(u, cx)
                                    }))
                                    .separator()
                                    .item(item("Sposta su", |w, u, cx| w.setlist_shift(u, -1, cx)))
                                    .item(item("Sposta giù", |w, u, cx| w.setlist_shift(u, 1, cx)))
                                    .item(item("Azzera inizio e fine", |w, u, cx| {
                                        w.setlist_clear_points(u, cx)
                                    }))
                                    .separator()
                                    .item(item("Togli dalla scaletta", |w, u, cx| {
                                        w.setlist_remove(u, cx)
                                    }))
                                }
                            })
                            .into_any_element(),
                    )
                })
                .collect();

        let body: AnyElement = if rows.is_empty() {
            div()
                .flex_1()
                .p_4()
                .text_color(p.muted)
                .child(
                    "Scaletta vuota. Aggiungi brani con ⌘⇧Invio o dal menu del tasto destro; \
                     doppio clic o Invio li mandano subito nel deck B.",
                )
                .into_any_element()
        } else {
            div()
                .id("setlist-rows")
                .flex_1()
                .overflow_y_scroll()
                .children(rows)
                .into_any_element()
        };

        div()
            .id("setlist")
            .key_context(SETLIST)
            .track_focus(&self.setlist_focus)
            .size_full()
            .flex()
            .flex_col()
            .bg(p.card)
            .on_action(
                cx.listener(|this, _: &SetlistPrevious, _, cx| this.setlist_move_cursor(-1, cx)),
            )
            .on_action(cx.listener(|this, _: &SetlistNext, _, cx| this.setlist_move_cursor(1, cx)))
            .on_action(cx.listener(|this, _: &SetlistLoad, _, cx| {
                this.with_setlist_cursor(|w, u| w.setlist_load(u, cx))
            }))
            .on_action(cx.listener(|this, _: &SetlistRemove, _, cx| {
                this.with_setlist_cursor(|w, u| w.setlist_remove(u, cx))
            }))
            .on_action(cx.listener(|this, _: &SetlistMoveUp, _, cx| {
                this.with_setlist_cursor(|w, u| w.setlist_shift(u, -1, cx))
            }))
            .on_action(cx.listener(|this, _: &SetlistMoveDown, _, cx| {
                this.with_setlist_cursor(|w, u| w.setlist_shift(u, 1, cx))
            }))
            .child(header)
            .child(columns)
            .child(body)
            .into_any_element()
    }

    pub(super) fn render_dialog(
        &self,
        dialog: Dialog,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (title, body, actions): (&str, AnyElement, Vec<AnyElement>) = match dialog {
            Dialog::Name { rename, .. } => (
                if rename.is_some() {
                    "RINOMINA SCALETTA"
                } else {
                    "SALVA SCALETTA COME…"
                },
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(caps("NOME", p.muted))
                    .child(Input::new(&self.name_input))
                    .into_any_element(),
                vec![
                    cancel(p, cx),
                    filled("name-ok", "SALVA", p.cue, p)
                        .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                            this.confirm_name(window, cx)
                        }))
                        .into_any_element(),
                ],
            ),
            Dialog::Unsaved(then) => (
                "MODIFICHE NON SALVATE",
                div()
                    .child(format!(
                        "La scaletta «{}» ha modifiche non salvate. Le salvo prima di {}?",
                        self.playlist.name.as_deref().unwrap_or("Senza nome"),
                        match then {
                            Then::New => "crearne una nuova",
                            Then::Open(_) => "aprire l'altra",
                        }
                    ))
                    .into_any_element(),
                vec![
                    cancel(p, cx),
                    button("unsaved-drop", "NON SALVARE", p)
                        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                            this.close_dialog(window, cx);
                            this.run_then(then, cx);
                        }))
                        .into_any_element(),
                    filled("unsaved-save", "SALVA", p.cue, p)
                        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                            this.save_setlist(Some(then), window, cx)
                        }))
                        .into_any_element(),
                ],
            ),
            Dialog::ConfirmDelete(list) => {
                let id = list.id;
                (
                    "ELIMINA SCALETTA",
                    div()
                        .child(format!(
                            "Eliminare la scaletta «{}»? Non si può annullare.",
                            list.name
                        ))
                        .into_any_element(),
                    vec![
                        button("del-back", "ANNULLA", p)
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                this.show_dialog(Dialog::Open, window, cx)
                            }))
                            .into_any_element(),
                        filled("del-ok", "ELIMINA", p.live, p)
                            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                                this.delete_setlist(id, window, cx)
                            }))
                            .into_any_element(),
                    ],
                )
            }
            Dialog::Open => {
                let rows: Vec<AnyElement> = self
                    .saved_lists
                    .iter()
                    .map(|list| {
                        let (id, l1, l2) = (list.id, list.clone(), list.clone());
                        let open = self.playlist.id == Some(id);
                        div()
                            .id(("saved", id as usize))
                            .flex()
                            .items_center()
                            .gap_2()
                            .px_2()
                            .py_1()
                            .rounded(px(RADIUS))
                            .hover(|s| s.bg(p.panel))
                            .child(
                                div()
                                    .id(("saved-name", id as usize))
                                    .flex_1()
                                    .cursor_pointer()
                                    .when(open, |d| d.font_weight(FontWeight::BOLD))
                                    .child(if open {
                                        format!("{} (aperta)", l1.name)
                                    } else {
                                        l1.name.clone()
                                    })
                                    .on_click(cx.listener(
                                        move |this, _: &ClickEvent, window, cx| {
                                            this.replace_setlist(Then::Open(id), window, cx)
                                        },
                                    )),
                            )
                            .child(
                                small_button(("saved-rename", id as usize), "RINOMINA", p)
                                    .on_click(cx.listener(
                                        move |this, _: &ClickEvent, window, cx| {
                                            this.show_dialog(
                                                Dialog::Name {
                                                    rename: Some(id),
                                                    then: None,
                                                },
                                                window,
                                                cx,
                                            )
                                        },
                                    )),
                            )
                            .child(
                                small_button(("saved-delete", id as usize), "ELIMINA", p).on_click(
                                    cx.listener(move |this, _: &ClickEvent, window, cx| {
                                        this.show_dialog(
                                            Dialog::ConfirmDelete(l2.clone()),
                                            window,
                                            cx,
                                        )
                                    }),
                                ),
                            )
                            .into_any_element()
                    })
                    .collect();
                (
                    "APRI SCALETTA",
                    if rows.is_empty() {
                        div()
                            .text_color(p.muted)
                            .child("Nessuna scaletta salvata. Prepara la scaletta e premi SALVA.")
                            .into_any_element()
                    } else {
                        div()
                            .id("saved-lists")
                            .flex()
                            .flex_col()
                            .gap(px(2.))
                            .max_h(px(320.))
                            .overflow_y_scroll()
                            .children(rows)
                            .into_any_element()
                    },
                    vec![cancel(p, cx)],
                )
            }
        };
        div()
            .id("dialog-backdrop")
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(gpui::hsla(0., 0., 0., 0.55))
            .on_click(cx.listener(|_, _: &ClickEvent, _, cx| cx.stop_propagation()))
            .child(
                div()
                    .id("dialog")
                    .w(px(460.))
                    .flex()
                    .flex_col()
                    .gap_4()
                    .p_5()
                    .rounded(px(RADIUS))
                    .bg(p.card)
                    .border_1()
                    .border_color(p.border)
                    .child(
                        div()
                            .font_family(DISPLAY_FONT)
                            .text_size(px(15.))
                            .font_weight(FontWeight::BOLD)
                            .child(title),
                    )
                    .child(body)
                    .when_some(self.dialog_error.clone(), |d, e| {
                        d.child(
                            div()
                                .p_2()
                                .rounded(px(RADIUS))
                                .bg(p.warn_bg)
                                .text_size(px(12.))
                                .child(SharedString::from(e)),
                        )
                    })
                    .child(div().flex().justify_end().gap_2().children(actions)),
            )
            .into_any_element()
    }
}

fn small_button(
    id: impl Into<gpui::ElementId>,
    label: &'static str,
    p: Palette,
) -> gpui::Stateful<gpui::Div> {
    button(id, label, p).h(px(24.)).px_2().text_size(px(10.5))
}

fn cancel(p: Palette, cx: &mut Context<Workspace>) -> AnyElement {
    button("dialog-cancel", "ANNULLA", p)
        .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.close_dialog(window, cx)))
        .into_any_element()
}
