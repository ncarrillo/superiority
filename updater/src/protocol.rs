use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Event {
    Checking,
    Available {
        version: String,
        title: String,
        notes: String,
        notes_format: String,
        size: u64,
    },
    Downloading {
        progress: f64,
    },
    Extracting {
        progress: f64,
    },
    Ready,
    Installing,
    Installed,
    NotFound {
        message: String,
    },
    Error {
        message: String,
    },
    Dismissed,
    Focus,
    QuitRequested,
}

impl Event {
    pub fn to_json(&self) -> serde_json::Result<String> {
        serde_json::to_string(self)
    }
}

#[cfg(test)]
mod tests {
    use super::Event;

    #[test]
    fn event_shape_matches_existing_bridge() {
        assert_eq!(
            Event::Downloading { progress: 0.5 }.to_json().unwrap(),
            r#"{"kind":"downloading","progress":0.5}"#
        );
        assert_eq!(Event::Ready.to_json().unwrap(), r#"{"kind":"ready"}"#);
    }
}
