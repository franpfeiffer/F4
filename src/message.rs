use iced::widget::text_editor;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub enum PendingAction {
    New,
    Open,
    Exit,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum VimMode {
    Normal,
    Insert,
    Command,
    Search,
    Visual,
    VisualLine,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LineNumbers {
    None,
    Absolute,
    Relative,
}

#[derive(Debug, Clone, PartialEq)]
pub enum VimPending {
    G,
    TextObjectModifier(char),
    ReplaceChar,
    FindChar,
}


#[derive(Debug, Clone)]
pub enum Message {
    Edit(text_editor::Action),
    New,
    Open,
    FileOpened(Option<(PathBuf, String)>),
    Save,
    SaveAs,
    FileSaved(Option<PathBuf>),
    Exit,
    Undo,
    Cut,
    Copy,
    Paste,
    Delete,
    SelectAll,
    FormatDocument,
    TogglePanel,
    ClosePanel,
    FindQueryChanged(String),
    ReplaceTextChanged(String),
    GoToLineChanged(String),
    ToggleCaseSensitive(bool),
    FindNext,
    FindPrevious,
    ReplaceOne,
    ReplaceAll,
    GoToLineSubmit,
    ToggleWordWrap,
    ZoomIn,
    ZoomOut,
    CtrlPressed,
    CtrlReleased,
    ShowAbout,
    CloseAbout,
    WindowCloseRequested,
    ConfirmSave,
    ConfirmDiscard,
    ConfirmCancel,
    VimKey(char),
    VimEnterInsert,
    VimEnterInsertAppend,
    VimEnterInsertLineStart,
    VimEnterInsertLineEnd,
    VimEnterInsertNewlineBelow,
    VimEnterInsertNewlineAbove,
    VimEnterNormal,
    VimEnterVisual,
    VimEnterVisualLine,
    VimEnterCommand,
    VimCommandChanged(String),
    VimCommandSubmit,
    ToggleVim,
    ToggleLineNumbers,
    VimEnterSearch(bool),
    VimSearchChanged(String),
    VimSearchSubmit,
    ToggleUndoPanel,
    UndoTreeSelect(usize),
    UndoTreeJump(usize),
    UndoPanelFocusToggle,
    UndoPanelMoveSelection(i32),
    UndoPanelConfirm,
    Redo,
    Tick,
}
