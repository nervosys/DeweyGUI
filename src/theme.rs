//! Theme system for Dewey.
//!
//! A token-based palette with built-in light and dark themes, for an
//! application to resolve into the [`Style`](crate::core::Style) values its
//! widgets take.
//!
//! **No widget reads an ambient theme.** Every widget is styled by the `Style`
//! it is given, so switching themes means an application resolving its own
//! tokens again — the runtime holds no current theme to switch. `Theme` is
//! also what plugins extend through
//! [`PluginContext`](crate::plugin::PluginContext), and it reaches the
//! application from there via
//! [`Model::plugins_ready`](crate::runtime::Model::plugins_ready).
//!
//! [`ThemeWatcher`] reloads a theme file when it changes on disk. Nothing in
//! this crate drives it. An application polls [`check()`](ThemeWatcher::check)
//! itself, from `update` or a tick.

use crate::core::Color;
use crate::ontology::*;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

/// Semantic color token for theme lookups.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ThemeToken {
    /// Primary brand color.
    Primary,
    /// Secondary brand color.
    Secondary,
    /// Accent / highlight color.
    Accent,
    /// General background.
    Background,
    /// Surface background (cards, panels).
    Surface,
    /// Default text color.
    Text,
    /// Muted/secondary text.
    TextMuted,
    /// Error / danger.
    Error,
    /// Warning.
    Warning,
    /// Success / positive.
    Success,
    /// Info.
    Info,
    /// Border / separator.
    Border,
    /// Widget focus ring.
    FocusRing,
    /// Disabled element.
    Disabled,
    /// Hover highlight.
    Hover,
    /// Selected / active element.
    Selected,
    /// Overlay / scrim background.
    Overlay,
    /// Shadow color.
    Shadow,
    /// Link / hyperlink color.
    Link,
}

/// A complete theme mapping tokens to colors.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Theme {
    /// Human-readable theme name.
    pub name: String,
    /// Token-to-color mappings.
    colors: HashMap<ThemeToken, Color>,
    /// Base font size in logical pixels.
    pub base_font_size: f32,
    /// Default border radius in logical pixels.
    pub border_radius: f32,
    /// Default spacing unit in logical pixels.
    pub spacing: f32,
}

impl Theme {
    /// Create a new theme with defaults for all tokens.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            colors: HashMap::new(),
            base_font_size: 14.0,
            border_radius: 4.0,
            spacing: 8.0,
        }
    }

    /// Set a token color.
    pub fn set(&mut self, token: ThemeToken, color: Color) {
        self.colors.insert(token, color);
    }

    /// Builder: set a token color.
    pub fn with(mut self, token: ThemeToken, color: Color) -> Self {
        self.colors.insert(token, color);
        self
    }

    /// Look up a token. Returns a fallback if not set.
    pub fn get(&self, token: ThemeToken) -> Color {
        self.colors.get(&token).copied().unwrap_or(Color::MAGENTA)
    }

    /// Set the base font size.
    pub fn with_font_size(mut self, size: f32) -> Self {
        self.base_font_size = size;
        self
    }

    /// Set the default border radius.
    pub fn with_border_radius(mut self, radius: f32) -> Self {
        self.border_radius = radius;
        self
    }

    /// Set the spacing unit.
    pub fn with_spacing(mut self, spacing: f32) -> Self {
        self.spacing = spacing;
        self
    }

    /// The built-in dark theme.
    pub fn dark() -> Self {
        Self::new("Dark")
            .with(ThemeToken::Primary, Color::rgba(0.35, 0.55, 0.95, 1.0))
            .with(ThemeToken::Secondary, Color::rgba(0.55, 0.35, 0.85, 1.0))
            .with(ThemeToken::Accent, Color::rgba(0.0, 0.85, 0.85, 1.0))
            .with(ThemeToken::Background, Color::rgba(0.08, 0.08, 0.10, 1.0))
            .with(ThemeToken::Surface, Color::rgba(0.12, 0.12, 0.15, 1.0))
            .with(ThemeToken::Text, Color::rgba(0.92, 0.92, 0.92, 1.0))
            .with(ThemeToken::TextMuted, Color::rgba(0.55, 0.55, 0.60, 1.0))
            .with(ThemeToken::Error, Color::rgba(0.90, 0.25, 0.25, 1.0))
            .with(ThemeToken::Warning, Color::rgba(0.90, 0.70, 0.20, 1.0))
            .with(ThemeToken::Success, Color::rgba(0.25, 0.80, 0.40, 1.0))
            .with(ThemeToken::Info, Color::rgba(0.35, 0.65, 0.95, 1.0))
            .with(ThemeToken::Border, Color::rgba(0.25, 0.25, 0.30, 1.0))
            .with(ThemeToken::FocusRing, Color::rgba(0.35, 0.55, 0.95, 0.7))
            .with(ThemeToken::Disabled, Color::rgba(0.35, 0.35, 0.40, 1.0))
            .with(ThemeToken::Hover, Color::rgba(1.0, 1.0, 1.0, 0.06))
            .with(ThemeToken::Selected, Color::rgba(0.35, 0.55, 0.95, 0.2))
            .with(ThemeToken::Overlay, Color::rgba(0.0, 0.0, 0.0, 0.6))
            .with(ThemeToken::Shadow, Color::rgba(0.0, 0.0, 0.0, 0.4))
            .with(ThemeToken::Link, Color::rgba(0.40, 0.65, 1.0, 1.0))
    }

    /// The built-in light theme.
    pub fn light() -> Self {
        Self::new("Light")
            .with(ThemeToken::Primary, Color::rgba(0.15, 0.40, 0.85, 1.0))
            .with(ThemeToken::Secondary, Color::rgba(0.45, 0.25, 0.75, 1.0))
            .with(ThemeToken::Accent, Color::rgba(0.0, 0.65, 0.65, 1.0))
            .with(ThemeToken::Background, Color::rgba(0.97, 0.97, 0.98, 1.0))
            .with(ThemeToken::Surface, Color::WHITE)
            .with(ThemeToken::Text, Color::rgba(0.10, 0.10, 0.12, 1.0))
            .with(ThemeToken::TextMuted, Color::rgba(0.45, 0.45, 0.50, 1.0))
            .with(ThemeToken::Error, Color::rgba(0.80, 0.15, 0.15, 1.0))
            .with(ThemeToken::Warning, Color::rgba(0.80, 0.60, 0.10, 1.0))
            .with(ThemeToken::Success, Color::rgba(0.15, 0.65, 0.30, 1.0))
            .with(ThemeToken::Info, Color::rgba(0.20, 0.50, 0.85, 1.0))
            .with(ThemeToken::Border, Color::rgba(0.82, 0.82, 0.85, 1.0))
            .with(ThemeToken::FocusRing, Color::rgba(0.15, 0.40, 0.85, 0.5))
            .with(ThemeToken::Disabled, Color::rgba(0.70, 0.70, 0.72, 1.0))
            .with(ThemeToken::Hover, Color::rgba(0.0, 0.0, 0.0, 0.04))
            .with(ThemeToken::Selected, Color::rgba(0.15, 0.40, 0.85, 0.12))
            .with(ThemeToken::Overlay, Color::rgba(0.0, 0.0, 0.0, 0.3))
            .with(ThemeToken::Shadow, Color::rgba(0.0, 0.0, 0.0, 0.12))
            .with(ThemeToken::Link, Color::rgba(0.10, 0.35, 0.80, 1.0))
    }

    /// Load a theme from a JSON file.
    ///
    /// Expected format:
    /// ```json
    /// {
    ///   "name": "My Theme",
    ///   "base_font_size": 14.0,
    ///   "border_radius": 4.0,
    ///   "spacing": 8.0,
    ///   "colors": { "Primary": [0.35, 0.55, 0.95, 1.0], ... }
    /// }
    /// ```
    pub fn load_from_json(path: &Path) -> Result<Self, String> {
        let data = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        serde_json::from_str(&data).map_err(|e| e.to_string())
    }

    /// Save this theme to a JSON file.
    pub fn save_to_json(&self, path: &Path) -> Result<(), String> {
        let data = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(path, data).map_err(|e| e.to_string())
    }
}

/// Watches a theme JSON file and reloads on change.
///
/// Poll [`check()`](ThemeWatcher::check) each frame; it returns `Some(Theme)`
/// when the file has been modified since the last load.
pub struct ThemeWatcher {
    path: std::path::PathBuf,
    last_modified: Arc<Mutex<Option<SystemTime>>>,
}

impl ThemeWatcher {
    /// Create a watcher for the given theme file path.
    pub fn new(path: impl Into<std::path::PathBuf>) -> Self {
        Self {
            path: path.into(),
            last_modified: Arc::new(Mutex::new(None)),
        }
    }

    /// Check if the theme file has been modified. Returns the new theme if so.
    pub fn check(&self) -> Option<Theme> {
        let meta = std::fs::metadata(&self.path).ok()?;
        let modified = meta.modified().ok()?;

        let mut last = self.last_modified.lock().ok()?;
        if *last == Some(modified) {
            return None;
        }
        *last = Some(modified);

        Theme::load_from_json(&self.path).ok()
    }
}

impl Discoverable for Theme {
    fn schema(&self) -> WidgetSchema {
        let mut schema = WidgetSchema::new(
            "Theme",
            "Token-based color and typography theme with dark/light presets",
            SemanticRole::System,
        );
        schema.usage_hint = Some("Theme::dark() or Theme::light()".into());
        schema.tags = vec![
            "theme".into(),
            "color".into(),
            "style".into(),
            "appearance".into(),
        ];
        schema
    }

    fn capabilities(&self) -> Vec<AgentCapability> {
        vec![]
    }

    fn actions(&self) -> Vec<AgentAction> {
        vec![
            AgentAction::with_params(
                "set_token",
                "Set a theme token color (r, g, b, a as 0.0-1.0)",
                vec![
                    ActionParam::required(
                        "token",
                        "Token name (e.g. Primary, Background)",
                        ActionParamType::String,
                    ),
                    ActionParam::required("r", "Red channel 0.0-1.0", ActionParamType::Float),
                    ActionParam::required("g", "Green channel 0.0-1.0", ActionParamType::Float),
                    ActionParam::required("b", "Blue channel 0.0-1.0", ActionParamType::Float),
                    ActionParam::optional(
                        "a",
                        "Alpha channel 0.0-1.0",
                        ActionParamType::Float,
                        serde_json::json!(1.0),
                    ),
                ],
                true,
            ),
            AgentAction::with_params(
                "get_token",
                "Get the color for a theme token",
                vec![ActionParam::required(
                    "token",
                    "Token name",
                    ActionParamType::String,
                )],
                false,
            ),
            AgentAction::simple(
                "list_tokens",
                "List all theme tokens and their values",
                false,
            ),
        ]
    }

    fn semantic_role(&self) -> SemanticRole {
        SemanticRole::System
    }

    fn agent_state(&self) -> serde_json::Value {
        let tokens: serde_json::Map<String, serde_json::Value> = self
            .colors
            .iter()
            .map(|(token, color)| {
                (
                    format!("{:?}", token),
                    serde_json::json!([color.r, color.g, color.b, color.a]),
                )
            })
            .collect();
        serde_json::json!({
            "name": self.name,
            "base_font_size": self.base_font_size,
            "border_radius": self.border_radius,
            "spacing": self.spacing,
            "token_count": self.colors.len(),
            "tokens": tokens,
        })
    }

    fn execute_action(
        &mut self,
        action: &str,
        params: &serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        match action {
            "set_token" => {
                let token_name = params["token"].as_str().ok_or("missing token")?;
                let token = parse_theme_token(token_name)?;
                let r = params["r"].as_f64().ok_or("missing r")? as f32;
                let g = params["g"].as_f64().ok_or("missing g")? as f32;
                let b = params["b"].as_f64().ok_or("missing b")? as f32;
                let a = params["a"].as_f64().unwrap_or(1.0) as f32;
                self.set(token, Color::rgba(r, g, b, a));
                Ok(serde_json::json!({ "set": token_name }))
            }
            "get_token" => {
                let token_name = params["token"].as_str().ok_or("missing token")?;
                let token = parse_theme_token(token_name)?;
                let c = self.get(token);
                Ok(serde_json::json!({ "token": token_name, "color": [c.r, c.g, c.b, c.a] }))
            }
            "list_tokens" => Ok(self.agent_state()),
            _ => Err(format!("Unknown action: {action}")),
        }
    }
}

fn parse_theme_token(name: &str) -> Result<ThemeToken, String> {
    match name {
        "Primary" => Ok(ThemeToken::Primary),
        "Secondary" => Ok(ThemeToken::Secondary),
        "Accent" => Ok(ThemeToken::Accent),
        "Background" => Ok(ThemeToken::Background),
        "Surface" => Ok(ThemeToken::Surface),
        "Text" => Ok(ThemeToken::Text),
        "TextMuted" => Ok(ThemeToken::TextMuted),
        "Error" => Ok(ThemeToken::Error),
        "Warning" => Ok(ThemeToken::Warning),
        "Success" => Ok(ThemeToken::Success),
        "Info" => Ok(ThemeToken::Info),
        "Border" => Ok(ThemeToken::Border),
        "FocusRing" => Ok(ThemeToken::FocusRing),
        "Disabled" => Ok(ThemeToken::Disabled),
        "Hover" => Ok(ThemeToken::Hover),
        "Selected" => Ok(ThemeToken::Selected),
        "Overlay" => Ok(ThemeToken::Overlay),
        "Shadow" => Ok(ThemeToken::Shadow),
        "Link" => Ok(ThemeToken::Link),
        _ => Err(format!("Unknown token: {name}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dark_theme_tokens() {
        let theme = Theme::dark();
        assert_eq!(theme.name, "Dark");
        let bg = theme.get(ThemeToken::Background);
        // Dark background should have low RGB values
        assert!(bg.r < 0.2);
        assert!(bg.g < 0.2);
    }

    #[test]
    fn light_theme_tokens() {
        let theme = Theme::light();
        let bg = theme.get(ThemeToken::Background);
        // Light background should have high RGB values
        assert!(bg.r > 0.9);
        assert!(bg.g > 0.9);
    }

    #[test]
    fn custom_theme() {
        let theme = Theme::new("Custom")
            .with(ThemeToken::Primary, Color::RED)
            .with_font_size(16.0)
            .with_border_radius(8.0);
        assert_eq!(theme.get(ThemeToken::Primary), Color::RED);
        assert_eq!(theme.base_font_size, 16.0);
        assert_eq!(theme.border_radius, 8.0);
    }
}
