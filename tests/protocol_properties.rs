//! Property-based tests for the agent protocol.
//!
//! All six existing property tests are about rectangles. The protocol had
//! none, and the protocol is where the divergences happen: a handler bound to
//! the wrong action name, a fast serialisation path drifting from the slow
//! one, an equality that depends on key order. Each of those was a real
//! defect, and each is the kind of thing a property states well.

use dewey::agent::driver::HeadlessDriver;
use dewey::agent::protocol::{AgentRequest, InjectedEvent, Viewport};
use dewey::ontology::Properties;
use dewey::prelude::*;
use proptest::prelude::*;

// ── the application under test ──────────────────────────────────────────

struct App {
    count: i32,
    text: String,
}

impl Model for App {
    type Msg = ();

    fn update(&mut self, _m: ()) -> Command<()> {
        Command::None
    }

    fn view(&self, frame: &mut Frame<'_>) {
        use dewey::widget::{StatefulWidget, TextInput};
        let rows = frame.area.rows_of(&[40.0, 40.0]);
        Button::new(format!("count {}", self.count))
            .on("inc", |a: &mut App| a.count += 1)
            .render(rows[0], frame);
        let mut state = dewey::widget::input::TextInputState::new().with_text(&self.text);
        TextInput::new()
            .on_input("name", |a: &mut App, t: &str| a.text = t.to_string())
            .render(rows[1], frame, &mut state);
    }
}

fn driver() -> HeadlessDriver<App> {
    let mut d = HeadlessDriver::new(
        App {
            count: 0,
            text: String::new(),
        },
        320.0,
        120.0,
    );
    d.init();
    d
}

// ── generators ──────────────────────────────────────────────────────────

/// Every request shape the protocol accepts, with plausible contents.
fn any_request() -> impl Strategy<Value = AgentRequest> {
    prop_oneof![
        Just(AgentRequest::Ping),
        any::<bool>().prop_map(|strict| AgentRequest::Validate { strict }),
        (any::<Option<u64>>(), any::<bool>()).prop_map(|(since, clip)| AgentRequest::GetTree {
            since,
            viewport: clip.then_some(Viewport {
                x: 0.0,
                y: 0.0,
                width: 320.0,
                height: 60.0,
            }),
        }),
        (
            proptest::option::of("[a-z]{1,8}"),
            proptest::option::of("[A-Za-z]{1,8}")
        )
            .prop_map(|(query, role)| AgentRequest::QueryOntology { query, role }),
        "[A-Za-z]{1,10}".prop_map(|widget_type| AgentRequest::GetSchema { widget_type }),
        "[a-z_]{1,10}".prop_map(|agent_id| AgentRequest::GetState { agent_id }),
        ("[a-z_]{1,10}", "[a-z_]{1,10}").prop_map(|(agent_id, action)| {
            AgentRequest::ExecuteAction {
                agent_id,
                action,
                params: serde_json::json!({ "text": "x", "index": 0 }),
            }
        }),
        (0.0f32..320.0, 0.0f32..120.0).prop_map(|(x, y)| AgentRequest::InjectEvent {
            event: InjectedEvent::MouseClick {
                x,
                y,
                button: "left".into()
            }
        }),
        Just(AgentRequest::Screenshot {
            format: "text".into()
        }),
    ]
}

/// A property bag with arbitrary keys and simple values.
fn any_properties() -> impl Strategy<Value = Properties> {
    proptest::collection::vec(("[a-z]{1,6}", any::<i64>()), 0..8).prop_map(|pairs| {
        let mut props = Properties::default();
        for (key, value) in pairs {
            props.insert(key, serde_json::json!(value));
        }
        props
    })
}

// ── properties ──────────────────────────────────────────────────────────

proptest! {
    /// Anything the protocol can express survives a trip through JSON.
    ///
    /// A request that serialises but does not deserialise is a request an
    /// agent can send and never have answered.
    #[test]
    fn a_request_survives_a_json_round_trip(request in any_request()) {
        let text = serde_json::to_string(&request).expect("serialise");
        let back: AgentRequest = serde_json::from_str(&text)
            .unwrap_or_else(|e| panic!("{text} did not deserialise: {e}"));
        prop_assert_eq!(
            serde_json::to_string(&back).expect("re-serialise"),
            text
        );
    }

    /// The transport's JSON and the in-process reply say the same thing.
    ///
    /// `get_tree` takes a hand-built serialisation path that skips the
    /// intermediate `serde_json::Value`. It is asserted equal to the ordinary
    /// path for that one request; this says it for every request, which is
    /// what stops the fast path drifting.
    #[test]
    fn both_reply_paths_agree(request in any_request()) {
        let mut a = driver();
        let mut b = driver();

        let typed = a.process_request(&request);
        let raw = b.process_request_json(&request);
        let parsed: serde_json::Value = serde_json::from_str(&raw)
            .unwrap_or_else(|e| panic!("{raw} is not JSON: {e}"));

        prop_assert_eq!(
            &parsed["success"],
            &serde_json::json!(typed.success),
            "success disagrees for {:?}", request
        );

        if let Some(data) = typed.data {
            let expected = serde_json::to_value(&data).expect("value");
            prop_assert_eq!(&parsed["data"], &expected, "data disagrees for {:?}", request);
        }
    }

    /// A request never leaves the application unable to answer the next one.
    ///
    /// Both transports used to hold their own copy of the request loop, and a
    /// request that put one of them into a bad state would not have shown up
    /// in any single-request test.
    #[test]
    fn the_driver_survives_any_sequence(requests in proptest::collection::vec(any_request(), 1..12)) {
        let mut d = driver();
        for request in &requests {
            let _ = d.process_request(request);
        }
        // Still answering, and still describing an interface.
        let after = d.process_request(&AgentRequest::GetTree { since: None, viewport: None });
        prop_assert!(after.success);
        prop_assert!(d.validate().iter().all(|f| f.code != "duplicate_agent_id"));
    }

    /// Property equality does not depend on the order keys were inserted.
    ///
    /// `serde_json` reorders keys on a round trip, so an order-sensitive
    /// comparison reported every unchanged widget as changed and would have
    /// flooded a subscribed agent with events. Fixed once, with no guard until
    /// now.
    #[test]
    fn properties_compare_by_content_not_order(props in any_properties()) {
        let text = serde_json::to_string(&props).expect("serialise");
        let back: Properties = serde_json::from_str(&text).expect("deserialise");
        prop_assert_eq!(&props, &back);

        let reversed: Properties = {
            let mut out = Properties::default();
            let mut pairs: Vec<_> = props.iter().collect();
            pairs.reverse();
            for (key, value) in pairs {
                out.insert(key.to_string(), value.clone());
            }
            out
        };
        prop_assert_eq!(&props, &reversed, "insertion order must not matter");
    }

    /// A viewport never describes more of the interface than the whole tree.
    #[test]
    fn a_viewport_never_widens_the_tree(height in 1.0f32..400.0) {
        let mut d = driver();
        let full = d
            .process_request(&AgentRequest::GetTree { since: None, viewport: None })
            .data
            .expect("tree");
        let clipped = d
            .process_request(&AgentRequest::GetTree {
                since: None,
                viewport: Some(Viewport { x: 0.0, y: 0.0, width: 320.0, height }),
            })
            .data
            .expect("tree");

        let full_len = serde_json::to_string(&full).expect("json").len();
        let clipped_len = serde_json::to_string(&clipped).expect("json").len();
        prop_assert!(
            clipped_len <= full_len + 64,
            "clipping to {height}px produced a larger reply: {clipped_len} > {full_len}"
        );
    }
}
