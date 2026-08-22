#[derive(Clone)]
pub(in crate::app::client) enum WarningDialog {
    Disconnected {
        detail: String,
    },
    Channel {
        title: String,
        detail: String,
        close_tab: Option<usize>,
    },
}

pub(in crate::app::client) fn compact_error(error: &str) -> String {
    let compact = error.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.chars().count() <= 72 {
        compact
    } else {
        format!("{}…", compact.chars().take(71).collect::<String>())
    }
}
