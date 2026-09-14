//! AI Chat — a ChatGPT/Claude-style conversational interface.
//!
//! Demonstrates:
//! - Multi-message conversation display with role-based styling
//! - Text input with send button
//! - Simulated streaming AI responses via async tasks
//! - Scroll area for chat history
//! - Keyboard shortcuts (Enter to send)
//!
//! Run with: `cargo run --example chat`

use dewey::event::KeyEventKind;
use dewey::prelude::*;
use dewey::widget::input::TextInputState;
use dewey::widget::scroll::ScrollState;
use std::cell::RefCell;
use std::time::Duration;

// ── Data model ──────────────────────────────────────────────────────

#[derive(Clone, Debug)]
enum Role {
    User,
    Assistant,
    System,
}

#[derive(Clone, Debug)]
struct ChatMessage {
    role: Role,
    content: String,
}

struct App {
    messages: Vec<ChatMessage>,
    input_state: RefCell<TextInputState>,
    scroll_state: RefCell<ScrollState>,
    is_generating: bool,
    partial_response: String,
    /// Stored bounds from the previous frame, for focusing the input by click.
    input_rect: RefCell<Rect>,
}

impl App {
    fn new() -> Self {
        let mut input = TextInputState::new();
        input.focused = true;
        Self {
            messages: vec![ChatMessage {
                role: Role::System,
                content: "Welcome! I'm Dewey Assistant. Ask me anything.".into(),
            }],
            input_state: RefCell::new(input),
            scroll_state: RefCell::new(ScrollState::new()),
            is_generating: false,
            partial_response: String::new(),
            input_rect: RefCell::new(Rect::ZERO),
        }
    }
}

// ── Messages ────────────────────────────────────────────────────────

#[derive(Debug)]
enum Msg {
    SendMessage,
    AppendToken(String),
    FinishResponse,
}

// ── Simulated AI responses ──────────────────────────────────────────

const RESPONSES: &[&str] = &[
    "That's an interesting question! Let me think about it...\n\n\
     The key insight is that modern GUI frameworks benefit enormously from \
     a clear separation between state management and rendering. The Elm \
     architecture achieves this by treating the UI as a pure function of \
     the application state.\n\n\
     This means every frame is predictable, testable, and easy to reason about.",
    "Great observation! Here are a few points to consider:\n\n\
     1. **Semantic ontology** gives AI agents structured access to every widget\n\
     2. **Headless drivers** enable testing without a GPU\n\
     3. **JSON Lines protocol** means any language can control the UI\n\n\
     Would you like me to elaborate on any of these?",
    "I'd be happy to help with that! The approach I'd recommend is:\n\n\
     First, define your `Model` with the state you need. Then implement \
     `update` to handle each message variant. Finally, `view` renders the \
     current state using the widget library.\n\n\
     The beauty of this pattern is that your UI is always a deterministic \
     function of your model — no hidden state, no surprises.",
    "Absolutely! Here's how I think about it:\n\n\
     - **Simplicity** — fewer concepts means fewer bugs\n\
     - **Composition** — small widgets compose into complex UIs\n\
     - **Testability** — pure functions are easy to verify\n\n\
     These principles have guided decades of functional UI research, and \
     they apply directly to building robust, agent-controllable interfaces.",
];

/// Pick a canned response based on the message count (cycles through them).
fn pick_response(index: usize) -> &'static str {
    RESPONSES[index % RESPONSES.len()]
}

// ── Model implementation ────────────────────────────────────────────

impl Model for App {
    type Msg = Msg;

    fn update(&mut self, msg: Msg) -> Command<Msg> {
        match msg {
            Msg::SendMessage => {
                let text = self.input_state.borrow().text.trim().to_string();
                if text.is_empty() || self.is_generating {
                    return Command::None;
                }

                // Add user message
                self.messages.push(ChatMessage {
                    role: Role::User,
                    content: text,
                });

                // Clear input but keep focus
                let mut new_state = TextInputState::new();
                new_state.focused = true;
                *self.input_state.borrow_mut() = new_state;

                // Start "generating"
                self.is_generating = true;
                self.partial_response.clear();

                // Simulate streaming: split the response into word-sized chunks
                // and deliver them as a sequence of AppendToken messages.
                let full_response = pick_response(self.messages.len()).to_string();
                let words: Vec<String> = full_response
                    .split_inclusive(char::is_whitespace)
                    .map(String::from)
                    .collect();

                // Build a batch of delayed token commands to simulate streaming
                let mut cmds: Vec<Command<Msg>> = Vec::new();
                let mut accumulated = String::new();
                for word in &words {
                    accumulated.push_str(word);
                    let snapshot = accumulated.clone();
                    cmds.push(Command::Task(Box::new(move || {
                        // Small delay per token to simulate network latency
                        std::thread::sleep(Duration::from_millis(30));
                        Msg::AppendToken(snapshot)
                    })));
                }
                cmds.push(Command::Task(Box::new(|| {
                    std::thread::sleep(Duration::from_millis(30));
                    Msg::FinishResponse
                })));

                // Fire the first token; the rest will come from the task queue.
                // Since tasks run sequentially in Dewey, we use Batch.
                Command::Batch(cmds)
            }

            Msg::AppendToken(text) => {
                self.partial_response = text;
                // Auto-scroll to bottom
                self.scroll_state.borrow_mut().offset_y += 20.0;
                Command::None
            }

            Msg::FinishResponse => {
                // Commit the full response as a message
                if !self.partial_response.is_empty() {
                    self.messages.push(ChatMessage {
                        role: Role::Assistant,
                        content: self.partial_response.clone(),
                    });
                }
                self.partial_response.clear();
                self.is_generating = false;
                // Auto-scroll to bottom
                self.scroll_state.borrow_mut().offset_y += 100.0;
                Command::None
            }
        }
    }

    fn view(&self, frame: &mut Frame<'_>) {
        let area = frame.area;

        // ┌─────────────────────────┐
        // │  Title bar              │  30px
        // ├─────────────────────────┤
        // │                         │
        // │  Chat messages (scroll) │  fill
        // │                         │
        // ├─────────────────────────┤
        // │  [Input          ][Send]│  40px
        // └─────────────────────────┘

        let rows = Layout::new(
            Direction::Vertical,
            [
                Constraint::Length(30.0),
                Constraint::Fill(1.0),
                Constraint::Length(40.0),
            ],
        )
        .split(area);

        // ── Title bar ───────────────────────────────────────────────
        Label::new("🤖  Dewey Assistant")
            .agent_id("chat_title")
            .style(Style::new().bg(Color::from_rgb8(30, 30, 46)))
            .render(rows[0], frame);

        // ── Chat history ────────────────────────────────────────────
        // Render each message as a label inside the scroll area.
        // We stack them vertically with role prefixes.

        let msg_area = rows[1].inner(&Margin::uniform(4.0));

        let line_height = 22.0;

        // Render scroll area
        ScrollArea::vertical()
            // `scroll_to` is advertised, so something has to answer it.
            .on_scroll("chat_scroll", |app: &mut App, _x, y| {
                if let Some(y) = y {
                    app.scroll_state.borrow_mut().offset_y = y;
                }
            })
            .render(msg_area, frame, &mut self.scroll_state.borrow_mut());

        // Render messages as labels inside the message area
        let mut y_offset = 0.0;
        for (i, msg) in self.messages.iter().enumerate() {
            let (prefix, color) = match msg.role {
                Role::User => ("You", Color::from_rgb8(130, 180, 255)),
                Role::Assistant => ("Dewey", Color::from_rgb8(160, 230, 160)),
                Role::System => ("System", Color::from_rgb8(200, 200, 130)),
            };

            // Role header
            let header_rect = Rect::new(
                msg_area.x,
                msg_area.y + y_offset,
                msg_area.width,
                line_height,
            );
            Label::new(format!("━━ {} ━━", prefix))
                .agent_id(format!("msg_{}_header", i))
                .style(Style::new().fg(color))
                .render(header_rect, frame);
            y_offset += line_height;

            // Message content
            let content_lines = (msg.content.len() as f32 / 60.0).ceil().max(1.0);
            let content_height = content_lines * line_height;
            let content_rect = Rect::new(
                msg_area.x + 8.0,
                msg_area.y + y_offset,
                msg_area.width - 16.0,
                content_height,
            );
            Label::new(&msg.content)
                .agent_id(format!("msg_{}_content", i))
                .style(Style::new().fg(Color::from_rgb8(220, 220, 220)))
                .render(content_rect, frame);
            y_offset += content_height + 8.0;
        }

        // Show partial response while "generating"
        if self.is_generating && !self.partial_response.is_empty() {
            let header_rect = Rect::new(
                msg_area.x,
                msg_area.y + y_offset,
                msg_area.width,
                line_height,
            );
            Label::new("━━ Dewey ━━  ⏳")
                .agent_id("msg_streaming_header")
                .style(Style::new().fg(Color::from_rgb8(160, 230, 160)))
                .render(header_rect, frame);
            y_offset += line_height;

            let content_lines = (self.partial_response.len() as f32 / 60.0).ceil().max(1.0);
            let content_height = content_lines * line_height;
            let content_rect = Rect::new(
                msg_area.x + 8.0,
                msg_area.y + y_offset,
                msg_area.width - 16.0,
                content_height,
            );
            Label::new(&self.partial_response)
                .agent_id("msg_streaming_content")
                .style(Style::new().fg(Color::from_rgb8(180, 220, 180)))
                .render(content_rect, frame);
        }

        // ── Input bar ───────────────────────────────────────────────
        let input_row = Layout::new(
            Direction::Horizontal,
            [Constraint::Fill(1.0), Constraint::Length(80.0)],
        )
        .split(rows[2]);

        // Stored for the click-to-focus path below.
        *self.input_rect.borrow_mut() = input_row[0];

        TextInput::new()
            .placeholder(if self.is_generating {
                "Waiting for response..."
            } else {
                "Type a message... (Enter to send)"
            })
            // Same for the field's own actions: an agent setting the text of
            // an unwired input is told it worked and types into nothing.
            .on_input("chat_input", |app: &mut App, text| {
                app.input_state.borrow_mut().text = text.to_string();
            })
            .render(input_row[0], frame, &mut self.input_state.borrow_mut());

        let send_label = if self.is_generating {
            "⏳"
        } else {
            "Send ↵"
        };
        Button::new(send_label)
            .action("send_btn", Msg::SendMessage)
            .enabled(!self.is_generating)
            .render(input_row[1], frame);
    }

    fn handle_event(&self, event: Event) -> Option<Msg> {
        match event {
            // Typed characters → insert into the input buffer
            Event::TextInput(ref text) => {
                if !self.is_generating {
                    let mut state = self.input_state.borrow_mut();
                    let cursor = state.cursor;
                    state.text.insert_str(cursor, text);
                    state.cursor = cursor + text.len();
                    state.focused = true;
                }
                None
            }

            // Enter key sends the message
            Event::Key(KeyEvent {
                code: KeyCode::Enter,
                kind: KeyEventKind::Press,
                modifiers,
            }) if modifiers.is_empty() => Some(Msg::SendMessage),

            // Backspace
            Event::Key(KeyEvent {
                code: KeyCode::Backspace,
                kind: KeyEventKind::Press,
                ..
            }) => {
                let mut state = self.input_state.borrow_mut();
                if state.cursor > 0 {
                    // Find previous char boundary
                    let new_cursor = state.text[..state.cursor]
                        .char_indices()
                        .next_back()
                        .map(|(i, _)| i)
                        .unwrap_or(0);
                    let old_cursor = state.cursor;
                    state.text.drain(new_cursor..old_cursor);
                    state.cursor = new_cursor;
                }
                None
            }

            // Delete
            Event::Key(KeyEvent {
                code: KeyCode::Delete,
                kind: KeyEventKind::Press,
                ..
            }) => {
                let mut state = self.input_state.borrow_mut();
                let cursor = state.cursor;
                if cursor < state.text.len() {
                    let next = state.text[cursor..]
                        .char_indices()
                        .nth(1)
                        .map(|(i, _)| cursor + i)
                        .unwrap_or(state.text.len());
                    state.text.drain(cursor..next);
                }
                None
            }

            // Arrow keys
            Event::Key(KeyEvent {
                code: KeyCode::Left,
                kind: KeyEventKind::Press,
                ..
            }) => {
                let mut state = self.input_state.borrow_mut();
                if state.cursor > 0 {
                    state.cursor = state.text[..state.cursor]
                        .char_indices()
                        .next_back()
                        .map(|(i, _)| i)
                        .unwrap_or(0);
                }
                None
            }
            Event::Key(KeyEvent {
                code: KeyCode::Right,
                kind: KeyEventKind::Press,
                ..
            }) => {
                let mut state = self.input_state.borrow_mut();
                if state.cursor < state.text.len() {
                    state.cursor = state.text[state.cursor..]
                        .char_indices()
                        .nth(1)
                        .map(|(i, _)| state.cursor + i)
                        .unwrap_or(state.text.len());
                }
                None
            }

            // Home / End
            Event::Key(KeyEvent {
                code: KeyCode::Home,
                kind: KeyEventKind::Press,
                ..
            }) => {
                self.input_state.borrow_mut().cursor = 0;
                None
            }
            Event::Key(KeyEvent {
                code: KeyCode::End,
                kind: KeyEventKind::Press,
                ..
            }) => {
                let mut state = self.input_state.borrow_mut();
                state.cursor = state.text.len();
                None
            }

            // Mouse click — send button or focus input
            Event::Mouse(ref mouse) if mouse.is_click() => {
                let input_rect = *self.input_rect.borrow();
                if input_rect.contains(mouse.position) {
                    self.input_state.borrow_mut().focused = true;
                }
                None
            }

            _ => None,
        }
    }

    fn register_ontology(&self, registry: &mut OntologyRegistry) {
        registry.register_schema(WidgetSchema::new(
            "ChatApp",
            "AI chat interface with streaming responses",
            SemanticRole::Container,
        ));
    }

    fn title(&self) -> &str {
        "Dewey — AI Chat"
    }
}

// ── Entry point ─────────────────────────────────────────────────────

fn main() -> std::result::Result<(), eframe::Error> {
    env_logger::init();
    log::info!("Starting Dewey AI Chat");

    Program::new(App::new())
        .with_options(ProgramOptions {
            width: 700.0,
            height: 550.0,
            ..Default::default()
        })
        .run()
}
