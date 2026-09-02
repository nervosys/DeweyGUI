//! Native file dialog abstraction for Dewey.
//!
//! **This module is types only.** It defines [`DialogBackend`] and the values
//! it trades in; it ships no platform implementation, and the runtime does not
//! call one. An application that wants a native file dialog implements the
//! trait itself — over `rfd` or similar — and calls it from its own
//! `Model::update`.
//!
//! [`NullDialogBackend`] exists for tests and for a build that should not open
//! a dialog; it answers every request with nothing.

use std::path::PathBuf;

use crate::ontology::*;

/// A file filter (e.g. "Images", &["png", "jpg"]).
#[derive(Debug, Clone)]
pub struct FileFilter {
    /// Display name for the filter.
    pub name: String,
    /// File extensions (without dot).
    pub extensions: Vec<String>,
}

impl FileFilter {
    /// Create a file filter.
    pub fn new(name: impl Into<String>, extensions: Vec<String>) -> Self {
        Self {
            name: name.into(),
            extensions,
        }
    }
}

/// Configuration for an open-file dialog.
#[derive(Debug, Clone, Default)]
pub struct OpenFileDialog {
    /// Dialog title.
    pub title: String,
    /// Starting directory.
    pub default_path: Option<PathBuf>,
    /// File filters.
    pub filters: Vec<FileFilter>,
    /// Allow multiple file selection.
    pub multiple: bool,
    /// Allow picking directories.
    pub directory: bool,
}

impl OpenFileDialog {
    /// Create an open-file dialog.
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            ..Default::default()
        }
    }

    /// Set the starting directory.
    pub fn with_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.default_path = Some(path.into());
        self
    }

    /// Add a file filter.
    pub fn with_filter(mut self, filter: FileFilter) -> Self {
        self.filters.push(filter);
        self
    }

    /// Allow multiple selection.
    pub fn with_multiple(mut self, multiple: bool) -> Self {
        self.multiple = multiple;
        self
    }

    /// Allow directory picking.
    pub fn with_directory(mut self, directory: bool) -> Self {
        self.directory = directory;
        self
    }
}

/// Configuration for a save-file dialog.
#[derive(Debug, Clone, Default)]
pub struct SaveFileDialog {
    /// Dialog title.
    pub title: String,
    /// Default file name.
    pub default_name: String,
    /// Starting directory.
    pub default_path: Option<PathBuf>,
    /// File filters.
    pub filters: Vec<FileFilter>,
}

impl SaveFileDialog {
    /// Create a save-file dialog.
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            ..Default::default()
        }
    }

    /// Set the default file name.
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.default_name = name.into();
        self
    }

    /// Set the starting directory.
    pub fn with_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.default_path = Some(path.into());
        self
    }

    /// Add a file filter.
    pub fn with_filter(mut self, filter: FileFilter) -> Self {
        self.filters.push(filter);
        self
    }
}

/// Result of a message box.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageBoxResult {
    Ok,
    Cancel,
    Yes,
    No,
}

/// A message box configuration.
#[derive(Debug, Clone)]
pub struct MessageBox {
    /// Title.
    pub title: String,
    /// Message body.
    pub message: String,
    /// Whether to show Cancel button.
    pub show_cancel: bool,
}

impl MessageBox {
    /// Create a message box.
    pub fn new(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            message: message.into(),
            show_cancel: false,
        }
    }

    /// Show a Cancel button.
    pub fn with_cancel(mut self) -> Self {
        self.show_cancel = true;
        self
    }
}

/// Trait for native dialog backends.
pub trait DialogBackend {
    /// Show an open-file dialog. Returns selected paths, or empty if cancelled.
    fn open_file(&self, config: &OpenFileDialog) -> Vec<PathBuf>;

    /// Show a save-file dialog. Returns the chosen path, or None if cancelled.
    fn save_file(&self, config: &SaveFileDialog) -> Option<PathBuf>;

    /// Show a message box. Returns the user's response.
    fn message_box(&self, config: &MessageBox) -> MessageBoxResult;
}

/// A stub dialog backend that always returns "cancelled" (for headless mode).
pub struct NullDialogBackend;

impl DialogBackend for NullDialogBackend {
    fn open_file(&self, _config: &OpenFileDialog) -> Vec<PathBuf> {
        Vec::new()
    }

    fn save_file(&self, _config: &SaveFileDialog) -> Option<PathBuf> {
        None
    }

    fn message_box(&self, _config: &MessageBox) -> MessageBoxResult {
        MessageBoxResult::Ok
    }
}

impl Discoverable for NullDialogBackend {
    fn schema(&self) -> WidgetSchema {
        let mut schema = WidgetSchema::new(
            "DialogBackend",
            "Native file and message dialog system for open/save/alert workflows",
            SemanticRole::Configuration,
        );
        schema.usage_hint = Some("backend.open_file(&OpenFileDialog::new(\"Open\"))".into());
        schema.tags = vec![
            "dialog".into(),
            "file".into(),
            "open".into(),
            "save".into(),
            "message".into(),
        ];
        schema
    }

    fn capabilities(&self) -> Vec<AgentCapability> {
        vec![AgentCapability::Clickable]
    }

    fn actions(&self) -> Vec<AgentAction> {
        vec![
            AgentAction::with_params(
                "open_file",
                "Show an open-file dialog",
                vec![
                    ActionParam::optional(
                        "title",
                        "Dialog title",
                        ActionParamType::String,
                        serde_json::json!("Open"),
                    ),
                    ActionParam::optional(
                        "multiple",
                        "Allow multiple selection",
                        ActionParamType::Boolean,
                        serde_json::json!(false),
                    ),
                    ActionParam::optional(
                        "directory",
                        "Allow directory selection",
                        ActionParamType::Boolean,
                        serde_json::json!(false),
                    ),
                ],
                false,
            ),
            AgentAction::with_params(
                "save_file",
                "Show a save-file dialog",
                vec![
                    ActionParam::optional(
                        "title",
                        "Dialog title",
                        ActionParamType::String,
                        serde_json::json!("Save"),
                    ),
                    ActionParam::optional(
                        "default_name",
                        "Default file name",
                        ActionParamType::String,
                        serde_json::json!(""),
                    ),
                ],
                false,
            ),
            AgentAction::with_params(
                "message_box",
                "Show a message box alert",
                vec![
                    ActionParam::required("title", "Message box title", ActionParamType::String),
                    ActionParam::required("message", "Message body", ActionParamType::String),
                    ActionParam::optional(
                        "show_cancel",
                        "Show cancel button",
                        ActionParamType::Boolean,
                        serde_json::json!(false),
                    ),
                ],
                false,
            ),
        ]
    }

    fn semantic_role(&self) -> SemanticRole {
        SemanticRole::Configuration
    }

    fn agent_state(&self) -> serde_json::Value {
        serde_json::json!({
            "backend": "null",
            "note": "Headless mode — dialogs return empty/cancelled results",
        })
    }

    fn execute_action(
        &mut self,
        action: &str,
        params: &serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        match action {
            "open_file" => {
                let title = params["title"].as_str().unwrap_or("Open");
                let multiple = params["multiple"].as_bool().unwrap_or(false);
                let directory = params["directory"].as_bool().unwrap_or(false);
                let config = OpenFileDialog::new(title)
                    .with_multiple(multiple)
                    .with_directory(directory);
                let result = self.open_file(&config);
                let paths: Vec<String> = result.iter().map(|p| p.display().to_string()).collect();
                Ok(serde_json::json!({ "paths": paths }))
            }
            "save_file" => {
                let title = params["title"].as_str().unwrap_or("Save");
                let name = params["default_name"].as_str().unwrap_or("");
                let config = SaveFileDialog::new(title).with_name(name);
                let result = self.save_file(&config);
                Ok(serde_json::json!({ "path": result.map(|p| p.display().to_string()) }))
            }
            "message_box" => {
                let title = params["title"].as_str().ok_or("missing title")?;
                let message = params["message"].as_str().ok_or("missing message")?;
                let mut config = MessageBox::new(title, message);
                if params["show_cancel"].as_bool().unwrap_or(false) {
                    config = config.with_cancel();
                }
                let result = self.message_box(&config);
                Ok(serde_json::json!({ "result": format!("{:?}", result) }))
            }
            _ => Err(format!("Unknown action: {action}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_dialog_returns_empty() {
        let backend = NullDialogBackend;
        let dialog = OpenFileDialog::new("Open").with_multiple(true);
        assert!(backend.open_file(&dialog).is_empty());

        let save = SaveFileDialog::new("Save").with_name("output.txt");
        assert!(backend.save_file(&save).is_none());

        let msg = MessageBox::new("Alert", "Hello").with_cancel();
        assert_eq!(backend.message_box(&msg), MessageBoxResult::Ok);
    }
}
