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

/// A subsystem trait, the file that owns it, and whether the crate drives it.
struct Subsystem {
    trait_name: &'static str,
    file: &'static str,
    /// Set when the crate ships no implementation and the module says so.
    types_only: bool,
}

const SUBSYSTEMS: &[Subsystem] = &[
    Subsystem {
        trait_name: "TrayBackend",
        file: "src/tray.rs",
        types_only: true,
    },
    Subsystem {
        trait_name: "DialogBackend",
        file: "src/dialog.rs",
        types_only: true,
    },
    Subsystem {
        trait_name: "Painter",
        file: "src/paint.rs",
        types_only: false,
    },
    Subsystem {
        trait_name: "Plugin",
        file: "src/plugin.rs",
        types_only: false,
    },
];

#[test]
fn a_subsystem_is_either_driven_or_declared_types_only() {
    for system in SUBSYSTEMS {
        let uses = references_outside(system.trait_name, system.file);
        let module = source(system.file);
        let declared = module.contains("types only") || module.contains("**This module is types only.**");

        if system.types_only {
            assert!(
                declared,
                "`{}` ships no implementation, so `{}` must say so at the top \
                 of the module — a downstream team built a tray around a trait \
                 the runtime never touches",
                system.trait_name, system.file
            );
        } else {
            assert!(
                uses > 0,
                "`{}` is advertised as a working subsystem but nothing outside \
                 {} refers to it",
                system.trait_name, system.file
            );
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
