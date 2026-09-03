//! An MCP server that answers questions about Dewey itself.
//!
//! Run with: `cargo run --release --example mcp_server`
//!
//! Most of the agent protocol is about a *running* application. This one
//! answers the questions asked before there is an application: what widgets
//! exist, what each one accepts, which one is the dropdown. The interface is
//! an empty program whose registry holds every built-in schema, so
//! `query_ontology` and `get_schema` are answered in full while `get_tree`
//! honestly returns nothing — there is nothing on screen.
//!
//! This is the server `benches/agentic/runner/mcp.json` attaches, and the
//! reason the benchmark has an `mcp` condition to compare against `bare`. It
//! is also the shape a coding agent wants while writing a Dewey application:
//! `initialize` hands it the instructions in `src/agent/mcp.rs`, which say the
//! application describes itself and where to start.
//!
//! Register it with a client by pointing at this command over stdio.

use dewey::prelude::*;

/// An application with no interface, so the catalogue is the whole answer.
struct Catalogue;

impl Model for Catalogue {
    type Msg = ();

    fn update(&mut self, _msg: ()) -> Command<()> {
        Command::None
    }

    fn view(&self, _frame: &mut Frame<'_>) {}

    fn register_ontology(&self, registry: &mut dewey::ontology::OntologyRegistry) {
        dewey::ontology::builtin::register_all(registry);
    }

    fn title(&self) -> &str {
        "Dewey widget catalogue"
    }
}

fn main() -> std::io::Result<()> {
    dewey::agent::mcp::McpServer::new(Catalogue, 1280.0, 720.0).run()
}
