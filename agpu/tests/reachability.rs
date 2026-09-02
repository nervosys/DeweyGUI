//! Modules this crate ships but does not drive must say so.
//!
//! Seven modules here have no reference outside themselves. For a library that
//! can be perfectly legitimate — a consumer uses `theme` or `animation`
//! directly and the crate need not. It is not legitimate when a module only
//! works once something integrates it, because then "compiles and is exported"
//! reads as "works".
//!
//! Three are in that second group. `multiwindow` tracks windows and opens
//! none; `accessibility` builds a tree in this crate's own vocabulary that
//! nothing converts to a platform API; `plugin` has a dispatch no runtime
//! calls. Each says so at the top of its file, and this checks that it still
//! does.

use std::path::Path;

fn source(relative: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {relative}: {e}"))
}

/// How many times a module is named outside its own file.
fn uses_outside(module: &str) -> usize {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut count = 0;
    let mut stack = vec![root];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).expect("read dir").flatten() {
            let path = entry.path();
            if path.is_dir() {
                if path.file_name().and_then(|n| n.to_str()) == Some(module) {
                    continue;
                }
                stack.push(path);
                continue;
            }
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default();
            if name != format!("{module}.rs") && name.ends_with(".rs") {
                let text = std::fs::read_to_string(&path).unwrap_or_default();
                count += text
                    .lines()
                    .filter(|l| {
                        let t = l.trim_start();
                        l.contains(&format!("{module}::"))
                            && !t.starts_with("//")
                            && !t.starts_with("pub mod")
                            && !t.starts_with("pub use")
                            && !t.starts_with("use ")
                    })
                    .count();
            }
        }
    }
    count
}

/// A module that needs integrating and is not integrated must admit it.
#[test]
fn an_unwired_module_says_it_is_unwired() {
    for module in ["multiwindow", "accessibility", "plugin"] {
        let text = source(&format!("src/{module}.rs"));
        let header: String = text
            .lines()
            .take_while(|l| l.starts_with("//!"))
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase();

        assert!(
            header.contains("does not")
                || header.contains("nothing in this crate")
                || header.contains("not accesskit"),
            "`{module}` has no caller in this crate and its module comment does \
             not say so. Either wire it or write down what it is, because \
             `pub mod` reads as `works`"
        );

        assert_eq!(
            uses_outside(module),
            0,
            "`{module}` now has a caller, so its comment saying nothing drives \
             it is out of date"
        );
    }
}

/// The crate-level example must compile.
///
/// It was `ignore`, so nothing checked it. The equivalent examples in the
/// sibling crate were ignored too, and all three of those called functions
/// that had never been written.
#[test]
fn the_crate_example_is_compiled_not_ignored() {
    let lib = source("src/lib.rs");
    let quick_start = lib
        .lines()
        .skip_while(|l| !l.contains("Quick Start"))
        .find(|l| l.trim_start().starts_with("//! ```"))
        .expect("a quick start example");

    assert!(
        !quick_start.contains("ignore"),
        "the quick start is ignored, so nothing compiles it: {quick_start}"
    );
    assert!(
        quick_start.contains("no_run"),
        "it opens a window, so it should be `no_run` — compiled, not executed"
    );
}
