//! Canvas drawing — interactive canvas that accumulates shapes.

use dewey::prelude::*;

struct App {
    shapes: Vec<Shape>,
    next_x: f32,
    hue: usize,
}

#[derive(Clone)]
struct Shape {
    kind: ShapeKind,
    x: f32,
    y: f32,
    color: [u8; 4],
}

#[derive(Clone)]
enum ShapeKind {
    Square,
    Circle,
}

impl App {
    fn new() -> Self {
        Self {
            shapes: Vec::new(),
            next_x: 10.0,
            hue: 0,
        }
    }

    fn palette(hue: usize) -> [u8; 4] {
        let colors: &[[u8; 4]] = &[
            [220, 50, 50, 255],
            [50, 180, 50, 255],
            [50, 100, 220, 255],
            [220, 180, 30, 255],
            [180, 50, 220, 255],
            [50, 200, 200, 255],
        ];
        colors[hue % colors.len()]
    }
}

#[derive(Debug)]
enum Msg {
    AddSquare,
    AddCircle,
    Clear,
}

impl Model for App {
    type Msg = Msg;

    fn update(&mut self, msg: Msg) -> Command<Msg> {
        match msg {
            Msg::AddSquare => {
                self.shapes.push(Shape {
                    kind: ShapeKind::Square,
                    x: self.next_x,
                    y: 10.0 + (self.shapes.len() as f32 * 5.0) % 60.0,
                    color: Self::palette(self.hue),
                });
                self.next_x += 30.0;
                self.hue += 1;
            }
            Msg::AddCircle => {
                self.shapes.push(Shape {
                    kind: ShapeKind::Circle,
                    x: self.next_x + 10.0,
                    y: 30.0 + (self.shapes.len() as f32 * 7.0) % 50.0,
                    color: Self::palette(self.hue),
                });
                self.next_x += 30.0;
                self.hue += 1;
            }
            Msg::Clear => {
                self.shapes.clear();
                self.next_x = 10.0;
                self.hue = 0;
            }
        }
        Command::None
    }

    fn view(&self, frame: &mut Frame<'_>) {
        let area = frame.area;

        let rows = Layout::new(
            Direction::Vertical,
            [Constraint::Length(30.0), Constraint::Fill(1.0)],
        )
        .split(area);

        // Button row
        let btn_cols = Layout::new(
            Direction::Horizontal,
            [
                Constraint::Length(120.0),
                Constraint::Length(120.0),
                Constraint::Length(120.0),
                Constraint::Fill(1.0),
            ],
        )
        .split(rows[0]);

        Button::new("Add Square [s]")
            .agent_id("btn_square")
            .render(btn_cols[0], frame);
        Button::new("Add Circle [c]")
            .agent_id("btn_circle")
            .render(btn_cols[1], frame);
        Button::new("Clear [x]")
            .agent_id("btn_clear")
            .render(btn_cols[2], frame);

        // Build canvas with all accumulated shapes
        let mut canvas = Canvas::new()
            // `clear` is advertised, so something has to answer it: an agent
            // calling it on an unwired canvas is told the call worked and the
            // drawing stays where it was.
            .on_clear("drawing_canvas", |app: &mut App| app.shapes.clear())
            .background([24, 24, 24, 255]);

        for shape in &self.shapes {
            canvas = match shape.kind {
                ShapeKind::Square => canvas.filled_rect(
                    shape.x,
                    shape.y,
                    shape.x + 20.0,
                    shape.y + 20.0,
                    shape.color,
                ),
                ShapeKind::Circle => canvas.filled_circle(shape.x, shape.y, 10.0, shape.color),
            };
        }

        canvas
            .text(
                5.0,
                85.0,
                format!("{} shapes", self.shapes.len()),
                11.0,
                [180, 180, 180, 255],
            )
            .render(rows[1], frame);
    }

    fn handle_event(&self, event: Event) -> Option<Msg> {
        match event {
            Event::Key(KeyEvent {
                code: KeyCode::Char('s'),
                ..
            }) => Some(Msg::AddSquare),
            Event::Key(KeyEvent {
                code: KeyCode::Char('c'),
                ..
            }) => Some(Msg::AddCircle),
            Event::Key(KeyEvent {
                code: KeyCode::Char('x'),
                ..
            }) => Some(Msg::Clear),
            _ => None,
        }
    }

    fn title(&self) -> &str {
        "Dewey — Canvas Drawing"
    }
}

fn main() -> std::result::Result<(), eframe::Error> {
    env_logger::init();
    Program::new(App::new())
        .with_options(ProgramOptions {
            width: 640.0,
            height: 480.0,
            ..Default::default()
        })
        .run()
}
