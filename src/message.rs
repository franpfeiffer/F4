use iced::widget::text_editor;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub enum PendingAction {
    New,
    Open,
    Exit,
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
}
