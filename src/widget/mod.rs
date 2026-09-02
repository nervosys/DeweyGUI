//! Widget system for Dewey.
//!
//! Widgets are the building blocks of a Dewey GUI. Each widget implements
//! the [`Widget`] trait and renders itself through the Painter abstraction.
//! Stateful widgets additionally track mutable per-instance state.

pub mod button;
pub mod canvas;
pub mod chart;
pub mod checkbox;
pub mod color_picker;
pub mod command_palette;
pub mod container;
pub mod date_picker;
pub mod image;
pub mod input;
pub mod label;
pub mod list;
pub mod menu;
pub mod modal;
pub mod panel;
pub mod progress;
pub mod radio;
pub mod rich_text;
pub mod scroll;
pub mod select;
pub mod slider;
pub mod splitter;
pub mod table;
pub mod tabs;
pub mod text_area;
pub mod toolbar;
pub mod tooltip;
pub mod tree;
pub mod virtual_list;

pub use button::Button;
pub use canvas::{Canvas, DrawCommand};
pub use chart::{Chart, ChartChange, ChartKind, Series};
pub use checkbox::Checkbox;
pub use color_picker::{ColorChange, ColorPicker, ColorPickerState};
pub use command_palette::{CommandPalette, CommandPaletteState, PaletteChange, PaletteCommand};
pub use container::Container;
pub use date_picker::{DateChange, DatePicker, DatePickerState, DateValue};
pub use image::{Image, ImageFit, ImageSource};
pub use input::{TextInput, TextInputState};
pub use label::Label;
pub use list::{List, ListState};
pub use menu::{Menu, MenuItem};
pub use modal::Modal;
pub use panel::Panel;
pub use progress::ProgressBar;
pub use radio::Radio;
pub use rich_text::{RichText, RichTextChange, TextSpan};
pub use scroll::{ScrollArea, ScrollState};
pub use select::{Select, SelectState};
pub use slider::{Slider, SliderState};
pub use splitter::{SplitDirection, Splitter, SplitterState};
pub use table::{SortDirection, Table, TableChange, TableState};
pub use tabs::{TabState, Tabs};
pub use text_area::{TextArea, TextAreaState};
pub use toolbar::{Toolbar, ToolbarItem};
pub use tooltip::Tooltip;
pub use tree::{Tree, TreeChange, TreeNode};
pub use virtual_list::{VirtualList, VirtualListState};

use crate::core::Rect;
use crate::ontology::Discoverable;

/// The core widget trait. All widgets must implement this.
///
/// Widgets follow the Elm architecture: they are created fresh each frame
/// with the current model data, rendered into a [`Rect`] area via a
/// [`Frame`](crate::runtime::Frame), and then dropped. Widgets are
/// **consumed** during rendering (moved into the frame).
///
/// Use this trait for widgets that carry no mutable state between frames—
/// e.g. labels, buttons, tooltips, and progress bars.
///
/// # Implementing
///
/// ```
/// use dewey::core::{Color, Position, Rect, Style};
/// use dewey::ontology::*;
/// use dewey::runtime::Frame;
/// use dewey::widget::Widget;
///
/// struct Badge {
///     text: String,
///     style: Style,
/// }
///
/// impl Widget for Badge {
///     fn render(self, area: Rect, frame: &mut Frame<'_>) {
///         let background = self.style.background.unwrap_or(Color::TRANSPARENT);
///         frame.painter().fill_rect(area, background, 4.0);
///         let text_style = self.style.resolved_text();
///         frame.painter().text(
///             Position::new(area.x + 8.0, area.y + 4.0),
///             &self.text,
///             &text_style,
///         );
///     }
/// }
/// # impl Discoverable for Badge {
/// #     fn schema(&self) -> WidgetSchema {
/// #         WidgetSchema::new("Badge", "A small label", SemanticRole::Display)
/// #     }
/// #     fn capabilities(&self) -> Vec<AgentCapability> { Vec::new() }
/// #     fn actions(&self) -> Vec<AgentAction> { Vec::new() }
/// #     fn semantic_role(&self) -> SemanticRole { SemanticRole::Display }
/// #     fn agent_state(&self) -> serde_json::Value { serde_json::json!({}) }
/// # }
/// ```
pub trait Widget: Discoverable {
    /// Render this widget into the given area.
    ///
    /// The `frame` provides access to the [`Painter`](crate::paint::Painter)
    /// for drawing primitives and the hit map for registering interactive regions.
    fn render(self, area: Rect, frame: &mut crate::runtime::Frame<'_>);
}

/// A stateful widget that separates persistent state from rendering.
///
/// Use this trait when a widget needs to maintain data across frames—
/// scroll offsets, text cursors, selected indices, open/close flags, etc.
/// The state lives in a `State` struct stored in the model, while the
/// widget itself is rebuilt each frame just like a regular [`Widget`].
///
/// # Implementing
///
/// ```
/// use dewey::core::{Position, Rect, Style};
/// use dewey::ontology::*;
/// use dewey::runtime::Frame;
/// use dewey::widget::StatefulWidget;
///
/// #[derive(Default)]
/// struct EditorState {
///     text: String,
///     last_area: Rect,
/// }
///
/// struct Editor;
///
/// impl StatefulWidget for Editor {
///     type State = EditorState;
///
///     fn render(self, area: Rect, frame: &mut Frame<'_>, state: &mut EditorState) {
///         // Read and mutate state as needed.
///         state.last_area = area;
///         let style = Style::default().resolved_text();
///         frame
///             .painter()
///             .text(Position::new(area.x, area.y), &state.text, &style);
///     }
/// }
/// # impl Discoverable for Editor {
/// #     fn schema(&self) -> WidgetSchema {
/// #         WidgetSchema::new("Editor", "A text editor", SemanticRole::Input)
/// #     }
/// #     fn capabilities(&self) -> Vec<AgentCapability> { Vec::new() }
/// #     fn actions(&self) -> Vec<AgentAction> { Vec::new() }
/// #     fn semantic_role(&self) -> SemanticRole { SemanticRole::Input }
/// #     fn agent_state(&self) -> serde_json::Value { serde_json::json!({}) }
/// # }
/// ```
pub trait StatefulWidget: Discoverable {
    /// The state type for this widget.
    type State;

    /// Render this stateful widget, reading and writing its mutable state.
    fn render(self, area: Rect, frame: &mut crate::runtime::Frame<'_>, state: &mut Self::State);
}
