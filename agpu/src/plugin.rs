//! A plugin registry.
//!
//! **Nothing in this crate calls it.** [`PluginRegistry::dispatch`] works,
//! and no runtime, app or event loop here invokes a lifecycle hook — an
//! embedder holding the registry has to dispatch to it itself.

use std::collections::HashMap;

/// Metadata describing a plugin.
#[derive(Debug, Clone)]
pub struct PluginMetadata {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
}

impl PluginMetadata {
    pub fn new(
        name: impl Into<String>,
        version: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
            description: description.into(),
            author: String::new(),
        }
    }

    pub fn author(mut self, author: impl Into<String>) -> Self {
        self.author = author.into();
        self
    }
}

/// Extension point that a plugin can provide.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ExtensionPoint {
    Widget,
    Renderer,
    Layout,
    Theme,
    InputHandler,
    Custom(String),
}

/// A registered extension from a plugin.
#[derive(Debug, Clone)]
pub struct Extension {
    pub point: ExtensionPoint,
    pub name: String,
    pub description: String,
}

/// Trait that all plugins must implement.
pub trait Plugin: std::fmt::Debug {
    /// Return metadata for this plugin.
    fn metadata(&self) -> PluginMetadata;

    /// Called when the plugin is loaded.
    fn on_load(&mut self) -> Result<(), String>;

    /// Called when the plugin is unloaded.
    fn on_unload(&mut self) -> Result<(), String>;

    /// Return the extensions this plugin provides.
    fn extensions(&self) -> Vec<Extension>;

    /// Handle a named action dispatched to this plugin.
    fn handle_action(
        &mut self,
        action: &str,
        params: &serde_json::Value,
    ) -> Result<serde_json::Value, String>;
}

/// State of a loaded plugin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginState {
    Registered,
    Loaded,
    Failed,
    Unloaded,
}

/// Entry tracking a plugin inside the manager.
struct PluginEntry {
    plugin: Box<dyn Plugin>,
    state: PluginState,
}

impl std::fmt::Debug for PluginEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PluginEntry")
            .field("metadata", &self.plugin.metadata())
            .field("state", &self.state)
            .finish()
    }
}

/// Manages registration, lifecycle, and lookup of plugins.
pub struct PluginManager {
    plugins: HashMap<String, PluginEntry>,
}

impl PluginManager {
    pub fn new() -> Self {
        Self {
            plugins: HashMap::new(),
        }
    }

    /// Register a plugin. Does not load it yet.
    pub fn register(&mut self, plugin: Box<dyn Plugin>) -> Result<(), String> {
        let meta = plugin.metadata();
        if self.plugins.contains_key(&meta.name) {
            return Err(format!("Plugin '{}' already registered", meta.name));
        }
        self.plugins.insert(
            meta.name.clone(),
            PluginEntry {
                plugin,
                state: PluginState::Registered,
            },
        );
        Ok(())
    }

    /// Load a registered plugin by name.
    pub fn load(&mut self, name: &str) -> Result<(), String> {
        let entry = self
            .plugins
            .get_mut(name)
            .ok_or_else(|| format!("Plugin '{name}' not found"))?;

        match entry.plugin.on_load() {
            Ok(()) => {
                entry.state = PluginState::Loaded;
                Ok(())
            }
            Err(e) => {
                entry.state = PluginState::Failed;
                Err(e)
            }
        }
    }

    /// Unload a loaded plugin by name.
    pub fn unload(&mut self, name: &str) -> Result<(), String> {
        let entry = self
            .plugins
            .get_mut(name)
            .ok_or_else(|| format!("Plugin '{name}' not found"))?;

        entry.plugin.on_unload()?;
        entry.state = PluginState::Unloaded;
        Ok(())
    }

    /// Remove a plugin entirely.
    pub fn remove(&mut self, name: &str) -> Result<(), String> {
        if self.plugins.remove(name).is_none() {
            return Err(format!("Plugin '{name}' not found"));
        }
        Ok(())
    }

    /// Get the state of a plugin.
    pub fn state(&self, name: &str) -> Option<PluginState> {
        self.plugins.get(name).map(|e| e.state)
    }

    /// List all registered plugin names.
    pub fn list(&self) -> Vec<String> {
        self.plugins.keys().cloned().collect()
    }

    /// Get all extensions of a specific type across loaded plugins.
    pub fn extensions_for(&self, point: &ExtensionPoint) -> Vec<Extension> {
        self.plugins
            .values()
            .filter(|e| e.state == PluginState::Loaded)
            .flat_map(|e| e.plugin.extensions())
            .filter(|ext| &ext.point == point)
            .collect()
    }

    /// Dispatch an action to a specific loaded plugin.
    pub fn dispatch(
        &mut self,
        plugin_name: &str,
        action: &str,
        params: &serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let entry = self
            .plugins
            .get_mut(plugin_name)
            .ok_or_else(|| format!("Plugin '{plugin_name}' not found"))?;

        if entry.state != PluginState::Loaded {
            return Err(format!("Plugin '{plugin_name}' is not loaded"));
        }

        entry.plugin.handle_action(action, params)
    }

    pub fn count(&self) -> usize {
        self.plugins.len()
    }
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[derive(Debug)]
    struct TestPlugin {
        loaded: bool,
    }

    impl TestPlugin {
        fn new() -> Self {
            Self { loaded: false }
        }
    }

    impl Plugin for TestPlugin {
        fn metadata(&self) -> PluginMetadata {
            PluginMetadata::new("test-plugin", "1.0.0", "A test plugin")
        }

        fn on_load(&mut self) -> Result<(), String> {
            self.loaded = true;
            Ok(())
        }

        fn on_unload(&mut self) -> Result<(), String> {
            self.loaded = false;
            Ok(())
        }

        fn extensions(&self) -> Vec<Extension> {
            vec![Extension {
                point: ExtensionPoint::Widget,
                name: "custom-widget".into(),
                description: "A custom widget".into(),
            }]
        }

        fn handle_action(
            &mut self,
            action: &str,
            _params: &serde_json::Value,
        ) -> Result<serde_json::Value, String> {
            match action {
                "ping" => Ok(json!("pong")),
                _ => Err(format!("Unknown action: {action}")),
            }
        }
    }

    #[derive(Debug)]
    struct FailPlugin;

    impl Plugin for FailPlugin {
        fn metadata(&self) -> PluginMetadata {
            PluginMetadata::new("fail-plugin", "0.1.0", "Always fails to load")
        }
        fn on_load(&mut self) -> Result<(), String> {
            Err("load error".into())
        }
        fn on_unload(&mut self) -> Result<(), String> {
            Ok(())
        }
        fn extensions(&self) -> Vec<Extension> {
            vec![]
        }
        fn handle_action(
            &mut self,
            _action: &str,
            _params: &serde_json::Value,
        ) -> Result<serde_json::Value, String> {
            Err("not loaded".into())
        }
    }

    #[test]
    fn register_plugin() {
        let mut mgr = PluginManager::new();
        assert!(mgr.register(Box::new(TestPlugin::new())).is_ok());
        assert_eq!(mgr.count(), 1);
    }

    #[test]
    fn duplicate_register_fails() {
        let mut mgr = PluginManager::new();
        mgr.register(Box::new(TestPlugin::new())).unwrap();
        assert!(mgr.register(Box::new(TestPlugin::new())).is_err());
    }

    #[test]
    fn load_plugin() {
        let mut mgr = PluginManager::new();
        mgr.register(Box::new(TestPlugin::new())).unwrap();
        mgr.load("test-plugin").unwrap();
        assert_eq!(mgr.state("test-plugin"), Some(PluginState::Loaded));
    }

    #[test]
    fn load_failure() {
        let mut mgr = PluginManager::new();
        mgr.register(Box::new(FailPlugin)).unwrap();
        assert!(mgr.load("fail-plugin").is_err());
        assert_eq!(mgr.state("fail-plugin"), Some(PluginState::Failed));
    }

    #[test]
    fn unload_plugin() {
        let mut mgr = PluginManager::new();
        mgr.register(Box::new(TestPlugin::new())).unwrap();
        mgr.load("test-plugin").unwrap();
        mgr.unload("test-plugin").unwrap();
        assert_eq!(mgr.state("test-plugin"), Some(PluginState::Unloaded));
    }

    #[test]
    fn remove_plugin() {
        let mut mgr = PluginManager::new();
        mgr.register(Box::new(TestPlugin::new())).unwrap();
        mgr.remove("test-plugin").unwrap();
        assert_eq!(mgr.count(), 0);
    }

    #[test]
    fn dispatch_action() {
        let mut mgr = PluginManager::new();
        mgr.register(Box::new(TestPlugin::new())).unwrap();
        mgr.load("test-plugin").unwrap();
        let result = mgr.dispatch("test-plugin", "ping", &json!(null)).unwrap();
        assert_eq!(result, json!("pong"));
    }

    #[test]
    fn dispatch_to_unloaded_fails() {
        let mut mgr = PluginManager::new();
        mgr.register(Box::new(TestPlugin::new())).unwrap();
        assert!(mgr.dispatch("test-plugin", "ping", &json!(null)).is_err());
    }

    #[test]
    fn extensions_listing() {
        let mut mgr = PluginManager::new();
        mgr.register(Box::new(TestPlugin::new())).unwrap();
        mgr.load("test-plugin").unwrap();
        let exts = mgr.extensions_for(&ExtensionPoint::Widget);
        assert_eq!(exts.len(), 1);
        assert_eq!(exts[0].name, "custom-widget");
    }

    #[test]
    fn extensions_only_loaded() {
        let mut mgr = PluginManager::new();
        mgr.register(Box::new(TestPlugin::new())).unwrap();
        // Not loaded
        let exts = mgr.extensions_for(&ExtensionPoint::Widget);
        assert_eq!(exts.len(), 0);
    }

    #[test]
    fn plugin_metadata_builder() {
        let meta = PluginMetadata::new("my-plugin", "2.0.0", "My plugin").author("Test Author");
        assert_eq!(meta.author, "Test Author");
    }
}
