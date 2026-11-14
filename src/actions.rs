//! Every action MusicJam dispatches and the keys bound to them.

use gpui::{KeyBinding, actions};

actions!(
    musicjam,
    [
        Quit,
        OpenSettings,
        Dismiss,
        FocusSearch,
        SelectPrevious,
        SelectNext,
        LoadSelected,
        PlayPause,
        Stop,
        TogglePreview,
        Crossfade,
        NudgeBack,
        NudgeForward,
        NudgeBackFine,
        NudgeForwardFine,
        Rescan,
        CycleTheme,
        AddToSetlist,
        NudgeEndBack,
        NudgeEndForward,
        SaveSetlist,
        SaveSetlistAs,
        NewSetlist,
        SetlistPrevious,
        SetlistNext,
        SetlistLoad,
        SetlistRemove,
        SetlistMoveUp,
        SetlistMoveDown,
    ]
);

pub const WORKSPACE: &str = "Workspace";
/// The search field: arrows there move through the results, not the caret.
const SEARCH_INPUT: &str = "Search > Input";
/// The setlist panel, once a row in it has been clicked.
pub const SETLIST: &str = "Setlist";

pub fn bindings() -> Vec<KeyBinding> {
    let ws = Some(WORKSPACE);
    let search = Some(SEARCH_INPUT);
    let setlist = Some(SETLIST);
    vec![
        KeyBinding::new("cmd-q", Quit, None),
        KeyBinding::new("cmd-,", OpenSettings, None),
        KeyBinding::new("cmd-f", FocusSearch, None),
        KeyBinding::new("cmd-r", Rescan, None),
        KeyBinding::new("cmd-t", CycleTheme, None),
        KeyBinding::new("cmd-p", PlayPause, None),
        KeyBinding::new("cmd-.", Stop, None),
        KeyBinding::new("cmd-e", TogglePreview, None),
        KeyBinding::new("cmd-enter", Crossfade, None),
        KeyBinding::new("escape", Dismiss, ws),
        KeyBinding::new("up", SelectPrevious, ws),
        KeyBinding::new("down", SelectNext, ws),
        KeyBinding::new("enter", LoadSelected, ws),
        KeyBinding::new("up", SelectPrevious, search),
        KeyBinding::new("down", SelectNext, search),
        KeyBinding::new("alt-left", NudgeBack, ws),
        KeyBinding::new("alt-right", NudgeForward, ws),
        KeyBinding::new("alt-shift-left", NudgeBackFine, ws),
        KeyBinding::new("alt-shift-right", NudgeForwardFine, ws),
        KeyBinding::new("alt-cmd-left", NudgeEndBack, ws),
        KeyBinding::new("alt-cmd-right", NudgeEndForward, ws),
        KeyBinding::new("cmd-shift-enter", AddToSetlist, None),
        KeyBinding::new("cmd-s", SaveSetlist, None),
        KeyBinding::new("cmd-shift-s", SaveSetlistAs, None),
        KeyBinding::new("cmd-n", NewSetlist, None),
        KeyBinding::new("up", SetlistPrevious, setlist),
        KeyBinding::new("down", SetlistNext, setlist),
        KeyBinding::new("enter", SetlistLoad, setlist),
        KeyBinding::new("backspace", SetlistRemove, setlist),
        KeyBinding::new("delete", SetlistRemove, setlist),
        KeyBinding::new("alt-cmd-up", SetlistMoveUp, setlist),
        KeyBinding::new("alt-cmd-down", SetlistMoveDown, setlist),
    ]
}
