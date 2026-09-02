//! System tray integration for Dewey.
//!
//! **This module is types only.** It defines [`TrayBackend`] and the values it
//! trades in; it ships no platform implementation, and the runtime does not
//! construct or poll one. An application that wants a tray icon implements
//! [`TrayBackend`] itself — over `tray-icon` or similar — and drives
//! [`TrayBackend::poll_event`] from its own `Model::update`.
//!
//! There is no `system-tray` feature to enable. A previous version of this
//! comment said the implementation was behind one, which sent people looking
//! for a feature that does not exist.
//!
//! To act on a tray click, return [`Command::SetWindowVisible`] or
//! [`Command::FocusWindow`] from `update`.
//!
//! [`Command::SetWindowVisible`]: crate::runtime::Command::SetWindowVisible
//! [`Command::FocusWindow`]: crate::runtime::Command::FocusWindow

use crate::ontology::*;

/// A menu item in the system tray context menu.
#[derive(Debug, Clone)]
pub enum TrayMenuItem {
    /// A clickable text item.
    Item {
        id: String,
        label: String,
        enabled: bool,
    },
    /// A separator line.
    Separator,
    /// A submenu.
    SubMenu {
        label: String,
        items: Vec<TrayMenuItem>,
    },
    /// A checkable item.
    CheckItem {
        id: String,
        label: String,
        checked: bool,
    },
}

impl TrayMenuItem {
    /// Create a simple menu item.
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self::Item {
            id: id.into(),
            label: label.into(),
            enabled: true,
        }
    }

    /// Create a separator.
    pub fn separator() -> Self {
        Self::Separator
    }

    /// Create a submenu.
    pub fn submenu(label: impl Into<String>, items: Vec<TrayMenuItem>) -> Self {
        Self::SubMenu {
            label: label.into(),
            items,
        }
    }

    /// Create a checkable item.
    pub fn check(id: impl Into<String>, label: impl Into<String>, checked: bool) -> Self {
        Self::CheckItem {
            id: id.into(),
            label: label.into(),
            checked,
        }
    }
}

/// Raw pixels for a tray icon.
///
/// Shaped like [`ImageData`](crate::paint::ImageData) so that supplying an
/// icon needs no image-decoding dependency: an application that has a PNG
/// decodes it however it likes and hands over the pixels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrayIconImage {
    pub width: u32,
    pub height: u32,
    /// RGBA, 8 bits per channel, `width * height * 4` bytes.
    pub rgba: Vec<u8>,
}

impl TrayIconImage {
    /// Build an icon from RGBA bytes.
    ///
    /// Returns `None` when the buffer is not `width * height * 4` bytes, which
    /// every platform tray API would otherwise reject at a less useful moment.
    #[must_use]
    pub fn from_rgba(width: u32, height: u32, rgba: Vec<u8>) -> Option<Self> {
        let expected = (width as usize) * (height as usize) * 4;
        (rgba.len() == expected).then_some(Self {
            width,
            height,
            rgba,
        })
    }
}

/// Configuration for a system tray icon.
#[derive(Debug, Clone)]
pub struct TrayConfig {
    /// Tooltip text shown on hover.
    pub tooltip: String,
    /// Context menu items.
    pub menu: Vec<TrayMenuItem>,
    /// The icon to show. `None` leaves the choice to the backend.
    ///
    /// Every platform tray API requires an icon to create the item at all, so
    /// a backend given `None` must invent one. Supply this if the application
    /// has artwork of its own.
    pub icon: Option<TrayIconImage>,
}

impl TrayConfig {
    /// Create tray config with a tooltip.
    pub fn new(tooltip: impl Into<String>) -> Self {
        Self {
            tooltip: tooltip.into(),
            menu: Vec::new(),
            icon: None,
        }
    }

    /// Set the context menu.
    pub fn with_menu(mut self, menu: Vec<TrayMenuItem>) -> Self {
        self.menu = menu;
        self
    }

    /// Set the tray icon.
    #[must_use]
    pub fn with_icon(mut self, icon: TrayIconImage) -> Self {
        self.icon = Some(icon);
        self
    }
}

/// Which mouse button a tray click used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayMouseButton {
    Left,
    Right,
    Middle,
}

/// Events from the system tray.
#[derive(Debug, Clone)]
pub enum TrayEvent {
    /// A menu item was clicked.
    MenuItemClicked(String),
    /// The tray icon was clicked once.
    ///
    /// Distinct from [`TrayEvent::DoubleClick`] because the platforms differ
    /// on which one means "show the window": on Windows it is a single left
    /// click, on macOS a single click opens the menu. Without this variant a
    /// backend had to report every click as a double one.
    Click { button: TrayMouseButton },
    /// The tray icon was double-clicked.
    DoubleClick,
}

/// Trait for system tray backends.
///
/// Implementors provide the platform-specific tray interaction.
pub trait TrayBackend {
    /// Show the tray icon with the given configuration.
    fn show(&mut self, config: &TrayConfig) -> Result<(), String>;

    /// Update the tooltip text.
    fn set_tooltip(&mut self, tooltip: &str) -> Result<(), String>;

    /// Update the context menu.
    fn set_menu(&mut self, menu: &[TrayMenuItem]) -> Result<(), String>;

    /// Hide and remove the tray icon.
    fn hide(&mut self) -> Result<(), String>;

    /// Poll for tray events (non-blocking).
    fn poll_event(&mut self) -> Option<TrayEvent>;
}

/// A stub tray backend that does nothing (for headless / test mode).
pub struct NullTrayBackend;

impl TrayBackend for NullTrayBackend {
    fn show(&mut self, _config: &TrayConfig) -> Result<(), String> {
        Ok(())
    }
    fn set_tooltip(&mut self, _tooltip: &str) -> Result<(), String> {
        Ok(())
    }
    fn set_menu(&mut self, _menu: &[TrayMenuItem]) -> Result<(), String> {
        Ok(())
    }
    fn hide(&mut self) -> Result<(), String> {
        Ok(())
    }
    fn poll_event(&mut self) -> Option<TrayEvent> {
        None
    }
}

impl Discoverable for NullTrayBackend {
    fn schema(&self) -> WidgetSchema {
        let mut schema = WidgetSchema::new(
            "TrayBackend",
            "System tray icon with context menu and event polling",
            SemanticRole::Configuration,
        );
        schema.usage_hint = Some("tray.show(&TrayConfig::new(\"tooltip\"))".into());
        schema.tags = vec![
            "tray".into(),
            "system".into(),
            "notification".into(),
            "icon".into(),
        ];
        schema
    }

    fn capabilities(&self) -> Vec<AgentCapability> {
        vec![AgentCapability::Clickable]
    }

    fn actions(&self) -> Vec<AgentAction> {
        vec![
            AgentAction::with_params(
                "show",
                "Show the system tray icon",
                vec![ActionParam::required(
                    "tooltip",
                    "Tooltip text",
                    ActionParamType::String,
                )],
                true,
            ),
            AgentAction::with_params(
                "set_tooltip",
                "Update the tray tooltip",
                vec![ActionParam::required(
                    "tooltip",
                    "Tooltip text",
                    ActionParamType::String,
                )],
                true,
            ),
            AgentAction::simple("hide", "Hide the tray icon", true),
            AgentAction::simple("poll_event", "Check for tray events", false),
        ]
    }

    fn semantic_role(&self) -> SemanticRole {
        SemanticRole::Configuration
    }

    fn agent_state(&self) -> serde_json::Value {
        serde_json::json!({
            "backend": "null",
            "note": "Headless mode — tray actions are no-ops",
        })
    }

    fn execute_action(
        &mut self,
        action: &str,
        params: &serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        match action {
            "show" => {
                let tooltip = params["tooltip"].as_str().ok_or("missing tooltip")?;
                let config = TrayConfig::new(tooltip);
                self.show(&config).map_err(|e| e.to_string())?;
                Ok(serde_json::json!({ "shown": true }))
            }
            "set_tooltip" => {
                let tooltip = params["tooltip"].as_str().ok_or("missing tooltip")?;
                self.set_tooltip(tooltip).map_err(|e| e.to_string())?;
                Ok(serde_json::json!({ "tooltip": tooltip }))
            }
            "hide" => {
                self.hide().map_err(|e| e.to_string())?;
                Ok(serde_json::json!({ "hidden": true }))
            }
            "poll_event" => {
                let event = self.poll_event();
                Ok(serde_json::json!({ "event": event.map(|e| format!("{:?}", e)) }))
            }
            _ => Err(format!("Unknown action: {action}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_tray_backend() {
        let mut tray = NullTrayBackend;
        let config = TrayConfig::new("Test App").with_menu(vec![
            TrayMenuItem::new("quit", "Quit"),
            TrayMenuItem::separator(),
            TrayMenuItem::check("dark", "Dark Mode", true),
        ]);
        tray.show(&config).unwrap();
        tray.set_tooltip("Updated").unwrap();
        assert!(tray.poll_event().is_none());
        tray.hide().unwrap();
    }
}
