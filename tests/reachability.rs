//! Every subsystem this crate advertises must either ship a working
//! implementation or say plainly that it does not.
//!
//! Three subsystems have been found unreachable by hand: the system tray
//! (reported downstream, after a team built around it), native file dialogs,
//! and the AccessKit bridge. Each compiled, each was documented, and none was
//! called from anywhere. The check that finds them is short enough to be a
//! test, so the fourth is caught here rather than by a user.

use std::path::Path;

/// Read a source file from the crate root.
fn source(relative: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {relative}: {e}"))
}

/// How many times a name appears outside the file that defines it.
fn references_outside(name: &str, own_file: &str) -> usize {
    let mut count = 0;
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut stack = vec![root];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).expect("read dir").flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                continue;
            }
            if path.ends_with(own_file) {
                continue;
            }
            let text = std::fs::read_to_string(&path).unwrap_or_default();
            // Neither a re-export nor a module declaration is a use: both say
            // the thing exists, not that anything drives it. Getting this
            // wrong made the first version of this test pass with the only
            // real call site deleted, because `pub mod accesskit_bridge;`
            // counted.
            count += text
                .lines()
                .filter(|line| {
                    let t = line.trim_start();
                    line.contains(name)
                        && !t.starts_with("pub use")
                        && !t.starts_with("use ")
                        && !t.starts_with("pub mod")
                        && !t.starts_with("mod ")
                        && !t.starts_with("//")
                })
                .count();
        }
    }
    count
}

/// What a module must be able to say for itself.
#[derive(PartialEq)]
enum Requirement {
    /// The crate drives it, and something outside its own file says so.
    Driven,
    /// The crate ships no implementation, and the module says so.
    TypesOnly,
    /// The implementation works and nothing here calls it. That is a fine
    /// thing for a utility an application reaches for, and a lie when the
    /// module's own summary describes it doing its job inside this crate:
    /// "arena-based allocation for per-frame temporaries" when no frame
    /// allocates from it, or a batch that "minimises draw calls" when nothing
    /// submits one.
    Undriven,
}

/// A subsystem, the file that owns it, and what that file has to say.
struct Subsystem {
    trait_name: &'static str,
    file: &'static str,
    requirement: Requirement,
}

const SUBSYSTEMS: &[Subsystem] = &[
    Subsystem {
        trait_name: "TrayBackend",
        file: "src/tray.rs",
        requirement: Requirement::TypesOnly,
    },
    Subsystem {
        trait_name: "DialogBackend",
        file: "src/dialog.rs",
        requirement: Requirement::TypesOnly,
    },
    Subsystem {
        trait_name: "Painter",
        file: "src/paint.rs",
        requirement: Requirement::Driven,
    },
    Subsystem {
        trait_name: "Plugin",
        file: "src/plugin.rs",
        requirement: Requirement::Driven,
    },
    Subsystem {
        trait_name: "Arena",
        file: "src/memory.rs",
        requirement: Requirement::Undriven,
    },
    Subsystem {
        trait_name: "RenderBatch",
        file: "src/gpu.rs",
        requirement: Requirement::Undriven,
    },
    Subsystem {
        trait_name: "ThemeWatcher",
        file: "src/theme.rs",
        requirement: Requirement::Undriven,
    },
];

/// The phrase an undriven module must carry, verbatim.
const UNDRIVEN: &str = "Nothing in this crate drives it.";

/// A module's doc comments as running prose, with the markers, the emphasis
/// and the line wrapping taken out.
///
/// A required sentence must be searchable without the author having to keep it
/// on one line — where rustfmt will not leave it anyway.
fn unwrapped_docs(module: &str) -> String {
    let words: Vec<&str> = module
        .lines()
        .map(str::trim_start)
        .filter_map(|l| l.strip_prefix("//!").or_else(|| l.strip_prefix("///")))
        .flat_map(str::split_whitespace)
        .collect();
    words.join(" ").replace("**", "")
}

#[test]
fn a_subsystem_is_either_driven_or_declared_types_only() {
    for system in SUBSYSTEMS {
        let uses = references_outside(system.trait_name, system.file);
        let module = source(system.file);

        match system.requirement {
            Requirement::TypesOnly => {
                let declared = module.contains("types only")
                    || module.contains("**This module is types only.**");
                assert!(
                    declared,
                    "`{}` ships no implementation, so `{}` must say so at the \
                     top of the module — a downstream team built a tray around \
                     a trait the runtime never touches",
                    system.trait_name, system.file
                );
            }
            Requirement::Driven => {
                assert!(
                    uses > 0,
                    "`{}` is advertised as a working subsystem but nothing \
                     outside {} refers to it",
                    system.trait_name,
                    system.file
                );
            }
            Requirement::Undriven => {
                assert!(
                    unwrapped_docs(&module).contains(UNDRIVEN),
                    "nothing in this crate calls `{}`, so `{}` must carry the \
                     line \"{UNDRIVEN}\". A working implementation nobody \
                     invokes optimises nothing, and its module summary should \
                     not read as though it does",
                    system.trait_name,
                    system.file
                );
                assert_eq!(
                    uses, 0,
                    "`{}` is now driven from somewhere, so drop the \
                     \"{UNDRIVEN}\" line from {} and move it to \
                     `Requirement::Driven` here",
                    system.trait_name, system.file
                );
            }
        }
    }
}

/// The AccessKit bridge is the one that was unreachable while looking wired.
#[test]
fn the_accesskit_bridge_is_called_by_something() {
    let uses = references_outside("accesskit_bridge", "src/accesskit_bridge.rs");
    assert!(
        uses > 0,
        "`accesskit_bridge` converts ontology nodes for a screen reader and \
         nothing called it, so applications published no accessibility tree \
         at all"
    );
}

/// A module that claims a cargo feature must name one that exists.
///
/// `src/tray.rs` pointed at a `system-tray` feature for its implementation.
/// There is no such feature, so anyone following the comment went looking for
/// something to enable and found nothing.
#[test]
fn documented_features_exist() {
    let manifest = source("Cargo.toml");
    let features: Vec<&str> = manifest
        .lines()
        .skip_while(|l| !l.starts_with("[features]"))
        .skip(1)
        .take_while(|l| !l.starts_with('['))
        .filter_map(|l| l.split('=').next())
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect();

    assert!(features.contains(&"accesskit"), "sanity: {features:?}");

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut stack = vec![root];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).expect("read dir").flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                continue;
            }
            let text = std::fs::read_to_string(&path).unwrap_or_default();
            for line in text.lines() {
                let trimmed = line.trim_start();
                if !trimmed.starts_with("//") {
                    continue;
                }
                // Only phrasing that presents the feature as available. A
                // line saying the feature does *not* exist is the fix, not
                // the fault.
                for named in ["system-tray", "native-dialogs"] {
                    let offered = line.contains(&format!("behind the `{named}` feature"))
                        || line.contains(&format!("enable the `{named}` feature"))
                        || line.contains(&format!("with `features = [\"{named}\"]`"));
                    assert!(
                        !offered,
                        "{} offers a `{named}` feature that does not exist: {}",
                        path.display(),
                        line.trim()
                    );
                }
            }
        }
    }
}

/// A subsystem driven by one backend and not the other is declared as such.
///
/// The plugin system shipped as a v1.2 framework feature and ran only under
/// `agpu-backend`, which is opt-in and off by default: `Program` had no
/// `with_plugin` at all, so `init`, `on_frame` and `on_shutdown` were never
/// called and a plugin's ontology registrations never reached an agent.
/// Neither backend opens a window in a test, which is why nothing noticed.
///
/// So the check is textual, and it is the same bargain the test above strikes:
/// drive it from both, or say in the roadmap that you do not. The profiler is
/// the one that currently says so.
#[test]
fn a_subsystem_is_driven_by_both_backends_or_declared_partial() {
    const DEFAULT: &str = "src/runtime/mod.rs";
    const AGPU: &str = "src/backend/agpu_backend.rs";

    /// A subsystem, and the symbol that shows a backend drives it.
    const DRIVEN: [(&str, &str); 4] = [
        ("plugin registration", "with_plugin"),
        ("plugin initialisation", "plugin::initialise"),
        ("the per-frame plugin hook", "on_frame()"),
        ("the plugin shutdown hook", "on_shutdown()"),
    ];

    let default = source(DEFAULT);
    let agpu = source(AGPU);

    for (name, symbol) in DRIVEN {
        for (backend, text) in [
            ("the default backend", &default),
            ("the agpu backend", &agpu),
        ] {
            assert!(
                text.contains(symbol),
                "{backend} never mentions `{symbol}`, so {name} does not \
                 happen there. A subsystem is only as real as the backend that \
                 drives it: drive it from both, or mark it `[~]` in ROADMAP.md \
                 and say which backend it works under"
            );
        }
    }

    // The known asymmetry, kept honest rather than silent.
    let roadmap = source("ROADMAP.md");
    assert!(
        !default.contains("Profiler"),
        "the default backend now drives the profiler, so the roadmap's `[~]` \
         for it is stale"
    );
    assert!(
        roadmap.contains("- [~] Profiling instrumentation"),
        "the profiler is driven by the agpu backend alone and the roadmap must \
         say so"
    );
}

/// A plugin's contributions survive the call that asks for them.
///
/// `initialise` builds an `I18n` and a `Theme` for plugins to write to. The
/// agpu backend built both in a block and dropped them at its end, so a
/// message catalogue or a theme extension — two of the four contributions the
/// module advertises — was discarded before the first frame.
#[test]
fn plugin_contributions_outlive_initialisation() {
    use dewey::core::Color;
    use dewey::i18n::MessageCatalog;
    use dewey::ontology::*;
    use dewey::plugin::{Plugin, PluginContext, PluginRegistry};
    use dewey::theme::ThemeToken;

    struct Contributor;

    impl Plugin for Contributor {
        fn name(&self) -> &str {
            "contributor"
        }

        fn init(&mut self, ctx: &mut PluginContext<'_>) {
            let mut catalogue = MessageCatalog::new();
            catalogue.insert("greeting", "hello");
            ctx.i18n.add_catalog("en", catalogue);
            ctx.theme
                .set(ThemeToken::Accent, Color::rgba(1.0, 0.0, 0.0, 1.0));
        }
    }

    let mut plugins = PluginRegistry::new();
    plugins.register(Contributor);
    let mut ontology = OntologyRegistry::new();

    let contributions = dewey::plugin::initialise(&mut plugins, &mut ontology);

    assert_eq!(
        contributions.i18n.t("greeting"),
        "hello",
        "the message catalogue a plugin registered did not survive `initialise`"
    );
    assert_eq!(
        contributions.theme.get(ThemeToken::Accent),
        Color::rgba(1.0, 0.0, 0.0, 1.0),
        "the theme token a plugin set did not survive `initialise`"
    );
}
