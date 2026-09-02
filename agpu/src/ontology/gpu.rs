//! GPU-specific ontology implementations.
//!
//! Provides [`Discoverable`] wrappers for GPU subsystems: the shape
//! renderer, text engine, surface, pipeline, and the agpu application
//! itself.

use super::*;

/// Ontology for the [`ShapeRenderer`](crate::renderer::ShapeRenderer).
pub struct ShapeRendererOntology;

impl Discoverable for ShapeRendererOntology {
    fn schema(&self) -> WidgetSchema {
        let mut s = WidgetSchema::new(
            "ShapeRenderer",
            "Batched 2D shape renderer — fills, strokes, circles, and lines",
            SemanticRole::System,
        );
        s.tags = vec![
            "gpu".into(),
            "renderer".into(),
            "2d".into(),
            "shapes".into(),
        ];
        s
    }

    fn capabilities(&self) -> Vec<AgentCapability> {
        vec![AgentCapability::Custom("gpu_renderer".into())]
    }

    fn actions(&self) -> Vec<AgentAction> {
        vec![
            AgentAction::simple("fill_rect", "Draw a filled rectangle", false),
            AgentAction::simple("stroke_rect", "Draw a stroked rectangle", false),
            AgentAction::simple("fill_circle", "Draw a filled circle", false),
            AgentAction::simple("stroke_circle", "Draw a stroked circle outline", false),
            AgentAction::simple("line", "Draw a line segment", false),
        ]
    }

    fn semantic_role(&self) -> SemanticRole {
        SemanticRole::System
    }

    fn agent_state(&self) -> serde_json::Value {
        serde_json::json!({ "kind": "ShapeRenderer" })
    }

    fn execute_action(
        &mut self,
        action: &str,
        _params: &serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        Err(format!("ShapeRenderer actions are internal: {action}"))
    }
}

/// Ontology for the [`TextEngine`](crate::text::TextEngine).
pub struct TextEngineOntology;

impl Discoverable for TextEngineOntology {
    fn schema(&self) -> WidgetSchema {
        let mut s = WidgetSchema::new(
            "TextEngine",
            "GPU text renderer (glyphon) — font shaping, glyph rasterisation",
            SemanticRole::System,
        );
        s.tags = vec!["gpu".into(), "text".into(), "font".into(), "glyphon".into()];
        s
    }

    fn capabilities(&self) -> Vec<AgentCapability> {
        vec![AgentCapability::Custom("gpu_text".into())]
    }

    fn actions(&self) -> Vec<AgentAction> {
        vec![
            AgentAction::simple("measure_text", "Measure text size without drawing", false),
            AgentAction::simple("draw_text", "Draw text at a position", false),
        ]
    }

    fn semantic_role(&self) -> SemanticRole {
        SemanticRole::System
    }

    fn agent_state(&self) -> serde_json::Value {
        serde_json::json!({ "kind": "TextEngine" })
    }

    fn execute_action(
        &mut self,
        action: &str,
        _params: &serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        Err(format!("TextEngine actions are internal: {action}"))
    }
}

/// Ontology for the GPU surface (swap chain).
pub struct SurfaceOntology;

impl Discoverable for SurfaceOntology {
    fn schema(&self) -> WidgetSchema {
        let mut s = WidgetSchema::new(
            "GpuSurface",
            "GPU presentation surface — swap chain and frame management",
            SemanticRole::System,
        );
        s.tags = vec!["gpu".into(), "surface".into(), "swap-chain".into()];
        s
    }

    fn capabilities(&self) -> Vec<AgentCapability> {
        vec![AgentCapability::Custom("gpu_surface".into())]
    }

    fn actions(&self) -> Vec<AgentAction> {
        vec![AgentAction::simple(
            "get_format",
            "Query the surface texture format",
            false,
        )]
    }

    fn semantic_role(&self) -> SemanticRole {
        SemanticRole::System
    }

    fn agent_state(&self) -> serde_json::Value {
        serde_json::json!({ "kind": "Surface" })
    }

    fn execute_action(
        &mut self,
        action: &str,
        _params: &serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        Err(format!("Surface actions are internal: {action}"))
    }
}

/// Ontology for a render/compute pipeline.
pub struct PipelineOntology;

impl Discoverable for PipelineOntology {
    fn schema(&self) -> WidgetSchema {
        let mut s = WidgetSchema::new(
            "GpuPipeline",
            "GPU pipeline — compiled shader program and fixed-function state",
            SemanticRole::System,
        );
        s.tags = vec!["gpu".into(), "pipeline".into(), "shader".into()];
        s
    }

    fn capabilities(&self) -> Vec<AgentCapability> {
        vec![AgentCapability::Custom("gpu_pipeline".into())]
    }

    fn actions(&self) -> Vec<AgentAction> {
        vec![AgentAction::simple(
            "get_info",
            "Query pipeline metadata",
            false,
        )]
    }

    fn semantic_role(&self) -> SemanticRole {
        SemanticRole::System
    }

    fn agent_state(&self) -> serde_json::Value {
        serde_json::json!({ "kind": "Pipeline" })
    }

    fn execute_action(
        &mut self,
        action: &str,
        _params: &serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        Err(format!("Pipeline actions are internal: {action}"))
    }
}

/// Ontology for the whole agpu application.
pub struct AgpuAppOntology;

impl Discoverable for AgpuAppOntology {
    fn schema(&self) -> WidgetSchema {
        let mut s = WidgetSchema::new(
            "AgpuApp",
            "agpu application runner — winit window + wgpu rendering loop",
            SemanticRole::Container,
        );
        s.tags = vec!["app".into(), "window".into(), "gpu".into()];
        s
    }

    fn capabilities(&self) -> Vec<AgentCapability> {
        vec![
            AgentCapability::Focusable,
            AgentCapability::Resizable {
                min_width: None,
                min_height: None,
                max_width: None,
                max_height: None,
            },
            AgentCapability::Closable,
        ]
    }

    fn actions(&self) -> Vec<AgentAction> {
        vec![
            AgentAction::simple("quit", "Close the application", false),
            AgentAction::simple("export_ontology", "Export ontology registry as JSON", false),
        ]
    }

    fn semantic_role(&self) -> SemanticRole {
        SemanticRole::Container
    }

    fn agent_state(&self) -> serde_json::Value {
        serde_json::json!({ "kind": "AgpuApp", "running": true })
    }

    fn execute_action(
        &mut self,
        action: &str,
        _params: &serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        match action {
            "quit" => Ok(serde_json::json!({"status": "quit_requested"})),
            "export_ontology" => Ok(serde_json::json!({"status": "export_requested"})),
            _ => Err(format!("Unknown action: {action}")),
        }
    }
}

/// Register every GPU schema this crate describes.
///
/// Called when an [`AgpuApp`](crate::app::AgpuApp) starts, so an agent that
/// asks what it is running on is told about the shape renderer, the text
/// engine, the surface, the pipeline and the application itself — not only
/// about the widgets the application happened to register.
pub fn register_gpu_ontology(registry: &mut crate::ontology::OntologyRegistry) {
    registry.register(&ShapeRendererOntology);
    registry.register(&TextEngineOntology);
    registry.register(&SurfaceOntology);
    registry.register(&PipelineOntology);
    registry.register(&AgpuAppOntology);
}

#[cfg(test)]
mod registration_tests {
    use super::*;
    use crate::ontology::OntologyRegistry;

    /// The crate's stated difference is that the GPU is discoverable too.
    ///
    /// The five schemas below existed, implemented `Discoverable`, and were
    /// exported in the prelude — and nothing registered any of them, so an
    /// agent querying an agpu application learned nothing about the renderer
    /// it was running on. The README called that "complete ontology".
    #[test]
    fn the_gpu_describes_itself() {
        let mut registry = OntologyRegistry::new();
        register_gpu_ontology(&mut registry);

        for name in [
            "ShapeRenderer",
            "TextEngine",
            "GpuSurface",
            "GpuPipeline",
            "AgpuApp",
        ] {
            let schema = registry
                .get_schema(name)
                .unwrap_or_else(|| panic!("`{name}` must be discoverable"));
            assert!(
                !schema.description.is_empty(),
                "`{name}` is registered with nothing to say about itself"
            );
        }
    }
}
