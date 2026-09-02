//! The schemas of every widget the library ships.
//!
//! Schemas were only ever registered by an application that chose to, so for
//! an ordinary program the type catalogue was empty: `get_schema("Button")`
//! returned nothing, and the diagnostics that cross-reference handlers against
//! advertised actions had nothing to compare against and quietly passed. An
//! agent asking what it could call on a widget was told nothing.
//!
//! Registering the catalogue costs one pass at startup, not per frame.

use super::OntologyRegistry;
use crate::widget::*;

/// Register the schema and actions of every built-in widget type.
///
/// Called for you when a program or headless driver starts. Registering a
/// schema twice is harmless — the later one replaces the earlier — so an
/// application is free to override any of these with its own.
pub fn register_all(registry: &mut OntologyRegistry) {
    registry.register(&Button::new("x"));
    registry.register(&Checkbox::new("x", false));
    registry.register(&TextInput::new());
    registry.register(&TextArea::new());
    registry.register(&Slider::new(0.0, 1.0));
    registry.register(&List::new(Vec::new()));
    registry.register(&Select::new("x", Vec::new()));
    registry.register(&Radio::new("x", false));
    registry.register(&Tabs::new(Vec::new()));
    registry.register(&Table::new(Vec::new(), Vec::new()));
    registry.register(&Tree::new(TreeNode::leaf("x")));
    registry.register(&Menu::new("x", Vec::new()));
    registry.register(&Toolbar::new(Vec::new()));
    registry.register(&Modal::new("x", false));
    registry.register(&DatePicker::new());
    registry.register(&ColorPicker::new("x"));
    registry.register(&CommandPalette::new(Vec::new()));
    registry.register(&ScrollArea::vertical());
    registry.register(&Splitter::new(crate::widget::SplitDirection::Vertical));
    registry.register(&Label::new("x"));
    registry.register(&Panel::new(crate::widget::panel::PanelSide::Left));
    registry.register(&Container::new());
    registry.register(&ProgressBar::new(0.0));
    registry.register(&Tooltip::new("x", "x"));
    registry.register(&RichText::new(Vec::new()));
    registry.register(&Chart::line("x"));
    registry.register(&Image::from_rgba(1, 1, vec![0, 0, 0, 0]));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every registered type must describe how to drive it, not merely exist.
    #[test]
    fn the_catalogue_carries_actions() {
        let mut registry = OntologyRegistry::new();
        register_all(&mut registry);

        for (name, action) in [
            ("Button", "click"),
            ("Checkbox", "toggle"),
            ("Table", "sort"),
            ("Tree", "expand_all"),
            ("Modal", "close"),
            ("CommandPalette", "execute"),
        ] {
            let schema = registry
                .get_schema(name)
                .unwrap_or_else(|| panic!("`{name}` is not in the catalogue"));
            assert!(
                schema.actions.iter().any(|a| a.name == action),
                "`{name}` does not advertise `{action}`"
            );
        }
    }
}
