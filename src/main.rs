mod actions;
mod audio;
mod library;
mod settings;
mod ui;

use gpui::{App, AppContext, Bounds, Menu, MenuItem, TitlebarOptions, WindowBounds, WindowOptions, px, size};
use gpui_component::Root;

use actions::*;
use ui::Workspace;

fn main() {
    gpui_platform::application().run(|cx: &mut App| {
        gpui_component::init(cx);
        cx.bind_keys(actions::bindings());
        cx.on_action(|_: &Quit, cx: &mut App| cx.quit());
        cx.set_menus(vec![
            Menu {
                name: "MusicJam".into(),
                disabled: false,
                items: vec![
                    MenuItem::action("Impostazioni…", OpenSettings),
                    MenuItem::separator(),
                    MenuItem::action("Esci da MusicJam", Quit),
                ],
            },
            Menu {
                name: "Libreria".into(),
                disabled: false,
                items: vec![MenuItem::action("Cerca", FocusSearch), MenuItem::action("Aggiorna libreria", Rescan)],
            },
            Menu {
                name: "Riproduzione".into(),
                disabled: false,
                items: vec![
                    MenuItem::action("Play / Pausa", PlayPause),
                    MenuItem::action("Stop", Stop),
                    MenuItem::action("Anteprima in cuffia", TogglePreview),
                    MenuItem::action("Crossfade", Crossfade),
                    MenuItem::separator(),
                    MenuItem::action("Partenza −1 s", NudgeBack),
                    MenuItem::action("Partenza +1 s", NudgeForward),
                ],
            },
        ]);

        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(None, size(px(1240.), px(780.)), cx))),
            window_min_size: Some(size(px(980.), px(560.))),
            titlebar: Some(TitlebarOptions { title: Some("MusicJam".into()), ..Default::default() }),
            app_id: Some("musicjam".into()),
            ..Default::default()
        };
        // Root must be the window's first layer, or the input's popups panic
        // looking for it.
        cx.open_window(options, |window, cx| {
            let workspace = cx.new(|cx| Workspace::new(window, cx));
            cx.new(|cx| Root::new(workspace, window, cx))
        })
        .expect("failed to open window");
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        cx.activate(true);
    });
}
