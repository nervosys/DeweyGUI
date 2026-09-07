//! A real system-tray icon on Windows, macOS and Linux.
//!
//! [`crate::tray`] defines the [`TrayBackend`] trait and a null implementation;
//! this is the platform half, over the `tray-icon` crate, behind the
//! `system-tray` feature.
//!
//! It is a backend, not a runtime integration: nothing here is constructed or
//! polled for you. An application owns a [`PlatformTray`], calls
//! [`TrayBackend::show`] once, and calls [`TrayBackend::poll_event`] from its
//! own `Model::update`.
//!
//! ```no_run
//! use dewey::tray::{PlatformTray, TrayBackend, TrayConfig, TrayEvent, TrayMenuItem};
//!
//! let mut tray = PlatformTray::new();
//! tray.show(&TrayConfig::new("My application").with_menu(vec![
//!     TrayMenuItem::new("show", "Show"),
//!     TrayMenuItem::separator(),
//!     TrayMenuItem::new("quit", "Quit"),
//! ]))?;
//!
//! // Once per frame, from `Model::update`:
//! if let Some(TrayEvent::MenuItemClicked(id)) = tray.poll_event() {
//!     println!("{id}");
//! }
//! # Ok::<(), String>(())
//! ```
//!
//! # Platform differences that matter
//!
//! On Windows and macOS the icon is created on the calling thread, which is the
//! UI thread, and that is where the platform wants it.
//!
//! On Linux the tray is a `StatusNotifierItem` published over D-Bus by
//! libappindicator, which needs a GTK main loop. winit's loop is not one, so
//! the icon is created on a thread of its own that runs `gtk::main`. Events
//! still arrive here, because `tray-icon` delivers them through process-wide
//! channels rather than per-icon ones.
//!
//! That split is why nothing on Linux builds a menu on the UI thread. `muda`
//! numbers menu items from a counter shared by the whole process, so building
//! the same menu twice yields two disjoint sets of ids — and only the ids of
//! the menu on screen ever reach [`TrayBackend::poll_event`]. A UI-thread copy
//! could therefore never decode a click; it would map every item to a bare
//! number, the application would match no arm, and the menu would open,
//! highlight, close and do nothing. So the GTK thread owns the only menu, and
//! the two sides trade a description of the menu one way and the mapping of
//! the menu that was built the other.
//!
//! If the desktop has no tray at all — a bare Wayland compositor, a headless
//! session — `show` returns an error and the caller carries on with a window
//! and no icon. A missing tray is a worse sidebar, not a broken program. On
//! Linux that answer has to be reached without asking GTK: with no display,
//! `gtk::init` does not report failure, it blocks for tens of seconds, so the
//! environment is read first and a session with no display is turned down
//! before a thread is spawned to wait on it.

use crate::tray::{
    TrayBackend, TrayConfig, TrayEvent, TrayIconImage, TrayMenuItem, TrayMouseButton,
};
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use tray_icon::{TrayIcon, TrayIconBuilder, TrayIconEvent};

#[cfg(all(unix, not(target_os = "macos")))]
use std::sync::{Arc, Mutex};

/// The platform tray.
#[derive(Default)]
pub struct PlatformTray {
    /// Held to keep the icon alive: dropping it removes the icon.
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    icon: Option<TrayIcon>,
    /// Menu ids in the order they were built, so a `MenuEvent` can be mapped
    /// back to the id the application chose.
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    ids: Vec<(String, String)>,
    /// The handle to the GTK thread that owns the icon and the only menu.
    #[cfg(all(unix, not(target_os = "macos")))]
    linux: Option<LinuxTray>,
}

/// The UI thread's end of the GTK thread.
///
/// Neither a menu nor an icon is `Send`, and GTK may only be touched from the
/// thread that initialised it, so nothing here is either. What crosses is a
/// description of the menu going out and the mapping of the menu that was
/// actually built coming back.
#[cfg(all(unix, not(target_os = "macos")))]
struct LinuxTray {
    /// Written by the GTK thread every time it builds a menu, read by
    /// [`TrayBackend::poll_event`]. It describes the menu on screen, which is
    /// the only menu whose ids a `MenuEvent` can carry.
    ids: Arc<Mutex<Vec<(String, String)>>>,
    /// Left here by the UI thread and picked up by the GTK thread on its next
    /// turn of the loop.
    pending: Arc<Mutex<Pending>>,
}

/// What the UI thread has asked the GTK thread to do.
#[cfg(all(unix, not(target_os = "macos")))]
#[derive(Default)]
struct Pending {
    menu: Option<Vec<TrayMenuItem>>,
    tooltip: Option<String>,
    hide: bool,
}

/// A poisoned tray mutex is not worth a panic: the data behind it is a menu.
#[cfg(all(unix, not(target_os = "macos")))]
fn lock<T>(cell: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    cell.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl PlatformTray {
    pub fn new() -> Self {
        Self::default()
    }

    /// The id the application chose for the platform id that was clicked.
    ///
    /// An unknown id comes back unchanged rather than being dropped, so a
    /// mismatch shows up as an unhandled id rather than as silence.
    #[cfg(all(unix, not(target_os = "macos")))]
    fn application_id(&self, raw: &str) -> String {
        match &self.linux {
            Some(linux) => lookup(&lock(&linux.ids), raw),
            None => raw.to_string(),
        }
    }

    #[cfg(any(target_os = "windows", target_os = "macos"))]
    fn application_id(&self, raw: &str) -> String {
        lookup(&self.ids, raw)
    }

    /// Put the icon on screen.
    ///
    /// Nothing is built here on Linux — see the note at the top of the module.
    #[cfg(all(unix, not(target_os = "macos")))]
    fn open(&mut self, config: &TrayConfig) -> Result<(), String> {
        // Asked rather than attempted: `gtk::init` on a session with no display
        // blocks for tens of seconds instead of returning an error, so waiting
        // for it to fail would cost the startup of every headless run and end
        // in a timeout that cannot tell "slow" from "impossible" anyway. This
        // is the same pair of variables winit checks before it gives up.
        if std::env::var_os("DISPLAY").is_none() && std::env::var_os("WAYLAND_DISPLAY").is_none() {
            return Err("no display, so no tray icon".to_string());
        }

        let linux = LinuxTray {
            ids: Arc::new(Mutex::new(Vec::new())),
            pending: Arc::new(Mutex::new(Pending::default())),
        };
        let ids = linux.ids.clone();
        let pending = linux.pending.clone();
        let items = config.menu.clone();
        let supplied = config.icon.clone();
        let tooltip = config.tooltip.clone();
        let (ready, started) = std::sync::mpsc::channel::<Result<(), String>>();

        std::thread::spawn(move || gtk_thread(items, supplied, tooltip, ids, pending, ready));
        self.linux = Some(linux);

        // Waiting is what turns "a thread was spawned" into "an icon exists",
        // which is the difference between returning Ok because something was
        // attempted and returning Ok because it worked.
        match started.recv_timeout(std::time::Duration::from_secs(5)) {
            Ok(result) => result,
            // The sender is dropped without a word only if the thread died.
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                Err("the tray thread stopped before it published an icon".to_string())
            }
            // Slow, not broken. The mapping is shared, so a menu that arrives
            // late still decodes its own clicks.
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                log::warn!("the tray is taking more than five seconds to appear");
                Ok(())
            }
        }
    }

    #[cfg(any(target_os = "windows", target_os = "macos"))]
    fn open(&mut self, config: &TrayConfig) -> Result<(), String> {
        let (menu, ids) = build_menu(&config.menu)?;
        self.ids = ids;
        let artwork = platform_icon(config.icon.as_ref())?;
        self.icon = Some(
            TrayIconBuilder::new()
                .with_menu(Box::new(menu))
                .with_tooltip(&config.tooltip)
                .with_icon(artwork)
                .build()
                .map_err(|error| format!("tray icon unavailable: {error}"))?,
        );
        Ok(())
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    fn retitle(&mut self, tooltip: &str) -> Result<(), String> {
        if let Some(linux) = &self.linux {
            lock(&linux.pending).tooltip = Some(tooltip.to_string());
        }
        Ok(())
    }

    #[cfg(any(target_os = "windows", target_os = "macos"))]
    fn retitle(&mut self, tooltip: &str) -> Result<(), String> {
        match &self.icon {
            Some(icon) => icon
                .set_tooltip(Some(tooltip))
                .map_err(|error| error.to_string()),
            None => Ok(()),
        }
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    fn rebuild(&mut self, items: &[TrayMenuItem]) -> Result<(), String> {
        if let Some(linux) = &self.linux {
            lock(&linux.pending).menu = Some(items.to_vec());
        }
        Ok(())
    }

    #[cfg(any(target_os = "windows", target_os = "macos"))]
    fn rebuild(&mut self, items: &[TrayMenuItem]) -> Result<(), String> {
        let (menu, ids) = build_menu(items)?;
        self.ids = ids;
        if let Some(icon) = &self.icon {
            icon.set_menu(Some(Box::new(menu)));
        }
        Ok(())
    }

    /// Dropping our end would not remove the Linux icon: the GTK thread owns
    /// it, so hiding has to be asked for rather than done.
    #[cfg(all(unix, not(target_os = "macos")))]
    fn close(&mut self) -> Result<(), String> {
        if let Some(linux) = self.linux.take() {
            lock(&linux.pending).hide = true;
        }
        Ok(())
    }

    #[cfg(any(target_os = "windows", target_os = "macos"))]
    fn close(&mut self) -> Result<(), String> {
        self.icon = None;
        Ok(())
    }
}

/// The mapping step on its own, so it can be tested without a tray.
fn lookup(ids: &[(String, String)], raw: &str) -> String {
    ids.iter()
        .find(|(platform, _)| platform == raw)
        .map(|(_, ours)| ours.clone())
        .unwrap_or_else(|| raw.to_string())
}

impl TrayBackend for PlatformTray {
    fn show(&mut self, config: &TrayConfig) -> Result<(), String> {
        self.open(config)
    }

    fn set_tooltip(&mut self, tooltip: &str) -> Result<(), String> {
        self.retitle(tooltip)
    }

    fn set_menu(&mut self, items: &[TrayMenuItem]) -> Result<(), String> {
        self.rebuild(items)
    }

    fn hide(&mut self) -> Result<(), String> {
        self.close()
    }

    /// Non-blocking: called once per frame from the UI loop.
    fn poll_event(&mut self) -> Option<TrayEvent> {
        if let Ok(event) = MenuEvent::receiver().try_recv() {
            return Some(TrayEvent::MenuItemClicked(
                self.application_id(&event.id().0),
            ));
        }

        match TrayIconEvent::receiver().try_recv() {
            Ok(TrayIconEvent::Click { button, .. }) => Some(TrayEvent::Click {
                button: match button {
                    tray_icon::MouseButton::Right => TrayMouseButton::Right,
                    tray_icon::MouseButton::Middle => TrayMouseButton::Middle,
                    tray_icon::MouseButton::Left => TrayMouseButton::Left,
                },
            }),
            Ok(TrayIconEvent::DoubleClick { .. }) => Some(TrayEvent::DoubleClick),
            _ => None,
        }
    }
}

/// The thread that owns the Linux icon, its menu, and a GTK main loop.
///
/// It reports once — through `ready` — whether an icon exists, and then serves
/// the UI thread's requests until asked to hide.
#[cfg(all(unix, not(target_os = "macos")))]
fn gtk_thread(
    items: Vec<TrayMenuItem>,
    supplied: Option<TrayIconImage>,
    tooltip: String,
    ids: Arc<Mutex<Vec<(String, String)>>>,
    pending: Arc<Mutex<Pending>>,
    ready: std::sync::mpsc::Sender<Result<(), String>>,
) {
    macro_rules! fail {
        ($($argument:tt)*) => {{
            let _ = ready.send(Err(format!($($argument)*)));
            return;
        }};
    }

    if gtk::init().is_err() {
        fail!("no GTK display, so no tray icon");
    }
    let (menu, mapping) = match build_menu(&items) {
        Ok(built) => built,
        Err(error) => fail!("the tray menu could not be built: {error}"),
    };
    let artwork = match platform_icon(supplied.as_ref()) {
        Ok(artwork) => artwork,
        Err(error) => fail!("{error}"),
    };
    let icon = match TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip(tooltip)
        .with_icon(artwork)
        .build()
    {
        Ok(icon) => icon,
        Err(error) => fail!("tray icon unavailable: {error}"),
    };

    // Published before the loop starts, so a click that arrives in the first
    // frame already has a menu to be looked up in.
    *lock(&ids) = mapping;
    let _ = ready.send(Ok(()));

    // `set_menu` and `set_tooltip` are called from the UI thread, which may not
    // touch GTK, so they leave the request behind and this collects it. A fifth
    // of a second is well under the time it takes to notice a stale menu, and
    // an idle tray costs five wakeups a second doing nothing.
    let mut icon: Option<TrayIcon> = Some(icon);
    gtk::glib::timeout_add_local(std::time::Duration::from_millis(200), move || {
        let request = std::mem::take(&mut *lock(&pending));
        let Some(current) = icon.as_ref() else {
            return gtk::glib::ControlFlow::Break;
        };

        if let Some(items) = request.menu {
            match build_menu(&items) {
                // The mapping goes with the menu: replacing one without the
                // other is how a working menu starts reporting bare numbers.
                Ok((menu, mapping)) => {
                    current.set_menu(Some(Box::new(menu)));
                    *lock(&ids) = mapping;
                }
                Err(error) => log::warn!("the tray menu could not be rebuilt: {error}"),
            }
        }
        if let Some(tooltip) = request.tooltip {
            if let Err(error) = current.set_tooltip(Some(&tooltip)) {
                log::warn!("the tray tooltip could not be set: {error}");
            }
        }
        if request.hide {
            // Dropping it is what removes the icon.
            icon = None;
            gtk::main_quit();
            return gtk::glib::ControlFlow::Break;
        }
        gtk::glib::ControlFlow::Continue
    });

    gtk::main();
}

/// Translate Dewey's menu description into a platform menu, keeping the
/// mapping from platform ids back to application ids.
fn build_menu(items: &[TrayMenuItem]) -> Result<(Menu, Vec<(String, String)>), String> {
    let menu = Menu::new();
    let mut ids = Vec::new();
    append(&menu, items, &mut ids)?;
    Ok((menu, ids))
}

fn append(
    menu: &Menu,
    items: &[TrayMenuItem],
    ids: &mut Vec<(String, String)>,
) -> Result<(), String> {
    for item in items {
        match item {
            TrayMenuItem::Separator => {
                menu.append(&PredefinedMenuItem::separator())
                    .map_err(|e| e.to_string())?;
            }
            TrayMenuItem::Item { id, label, enabled } => {
                let entry = MenuItem::new(label, *enabled, None);
                ids.push((entry.id().0.clone(), id.clone()));
                menu.append(&entry).map_err(|e| e.to_string())?;
            }
            // A check item without persistent state would lie the moment it was
            // clicked, so it is drawn as a plain item with the state in its
            // label. The label is rebuilt from the model on every `set_menu`.
            TrayMenuItem::CheckItem { id, label, checked } => {
                let mark = if *checked { "✓ " } else { "   " };
                let entry = MenuItem::new(format!("{mark}{label}"), true, None);
                ids.push((entry.id().0.clone(), id.clone()));
                menu.append(&entry).map_err(|e| e.to_string())?;
            }
            TrayMenuItem::SubMenu { label, items } => {
                let submenu = Submenu::new(label, true);
                append_submenu(&submenu, items, ids)?;
                menu.append(&submenu).map_err(|e| e.to_string())?;
            }
        }
    }
    Ok(())
}

/// The same walk as [`append`], against a submenu.
///
/// `Menu` and `Submenu` share no append trait in `tray-icon`, so a two-level
/// menu costs one duplicated loop rather than a generic parameter threaded
/// through everything that touches a menu.
fn append_submenu(
    submenu: &Submenu,
    items: &[TrayMenuItem],
    ids: &mut Vec<(String, String)>,
) -> Result<(), String> {
    for item in items {
        match item {
            TrayMenuItem::Separator => {
                submenu
                    .append(&PredefinedMenuItem::separator())
                    .map_err(|e| e.to_string())?;
            }
            TrayMenuItem::Item { id, label, enabled } => {
                let entry = MenuItem::new(label, *enabled, None);
                ids.push((entry.id().0.clone(), id.clone()));
                submenu.append(&entry).map_err(|e| e.to_string())?;
            }
            TrayMenuItem::CheckItem { id, label, checked } => {
                let mark = if *checked { "✓ " } else { "   " };
                let entry = MenuItem::new(format!("{mark}{label}"), true, None);
                ids.push((entry.id().0.clone(), id.clone()));
                submenu.append(&entry).map_err(|e| e.to_string())?;
            }
            // One level of nesting is as deep as a tray menu should go.
            TrayMenuItem::SubMenu { .. } => {}
        }
    }
    Ok(())
}

/// The icon to hand the platform: the application's own if it supplied one,
/// otherwise the fallback below.
///
/// Every platform tray API needs an icon to create the item at all, so a
/// backend given `None` has to produce something rather than fail.
fn platform_icon(supplied: Option<&TrayIconImage>) -> Result<tray_icon::Icon, String> {
    match supplied {
        Some(image) => {
            let expected = (image.width * image.height * 4) as usize;
            if image.rgba.len() != expected {
                return Err(format!(
                    "tray icon is {}x{}, which needs {expected} bytes of RGBA, but got {}",
                    image.width,
                    image.height,
                    image.rgba.len()
                ));
            }
            tray_icon::Icon::from_rgba(image.rgba.clone(), image.width, image.height)
                .map_err(|error| format!("tray icon rejected: {error}"))
        }
        None => Ok(fallback_icon()),
    }
}

/// The icon used when the application declared none.
///
/// A neutral rounded square. Every platform tray API needs an icon to create
/// the item at all, so a backend handed `None` has to draw something; this is
/// deliberately anonymous, because artwork is the application's business and a
/// framework guessing at it is worse than a placeholder that looks like one.
fn fallback_icon() -> tray_icon::Icon {
    const SIZE: u32 = 32;
    let mut rgba = vec![0u8; (SIZE * SIZE * 4) as usize];
    let body = [110u8, 118, 129, 255];
    let mark = [232u8, 236, 242, 255];

    for y in 0..SIZE {
        for x in 0..SIZE {
            // A 5px corner radius, clipped by the squared distance from the
            // corner's centre rather than by a trigonometric test.
            let corner_x = if x < 5 {
                5 - x
            } else if x >= SIZE - 5 {
                x - (SIZE - 6)
            } else {
                0
            };
            let corner_y = if y < 5 {
                5 - y
            } else if y >= SIZE - 5 {
                y - (SIZE - 6)
            } else {
                0
            };
            if corner_x * corner_x + corner_y * corner_y > 25 {
                continue;
            }

            let index = ((y * SIZE + x) * 4) as usize;
            rgba[index..index + 4].copy_from_slice(&body);

            // A single dot in the middle, so the icon reads as an application
            // rather than as a missing image.
            let dx = x as i32 - 16;
            let dy = y as i32 - 16;
            if dx * dx + dy * dy <= 16 {
                rgba[index..index + 4].copy_from_slice(&mark);
            }
        }
    }

    tray_icon::Icon::from_rgba(rgba, SIZE, SIZE).expect("the generated icon is well-formed")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The fallback exists because every platform API demands an icon, so the
    /// one thing it must never do is fail to build.
    #[test]
    fn the_fallback_icon_is_well_formed() {
        let _ = fallback_icon();
    }

    /// A menu round trips: every id an application named comes back out of the
    /// mapping, including the ones inside a submenu.
    #[test]
    fn every_menu_id_maps_back_to_the_one_the_application_chose() {
        let items = vec![
            TrayMenuItem::new("show", "Show"),
            TrayMenuItem::separator(),
            TrayMenuItem::check("float", "Always on top", true),
            TrayMenuItem::submenu("Go to", vec![TrayMenuItem::new("stack", "Stack")]),
        ];
        let (_menu, ids) = build_menu(&items).expect("menu builds");
        let ours: Vec<&str> = ids.iter().map(|(_, ours)| ours.as_str()).collect();
        assert_eq!(ours, ["show", "float", "stack"]);
    }

    /// The mapping is the whole of what a click means.
    ///
    /// An id that is not in it comes back unchanged rather than being dropped,
    /// so a mismatch reaches the application as an id nothing handles — which
    /// is at least visible — instead of as a menu that silently does nothing.
    #[test]
    fn an_unknown_platform_id_survives_the_lookup() {
        let ids = vec![("1017".to_string(), "quit".to_string())];
        assert_eq!(lookup(&ids, "1017"), "quit");
        assert_eq!(lookup(&ids, "1018"), "1018");
        assert_eq!(lookup(&[], "1017"), "1017");
    }

    /// Why the Linux tray builds its menu exactly once, on the GTK thread.
    ///
    /// `muda` numbers menu items from a counter shared by the whole process,
    /// so two builds of the same items have no id in common. A second copy
    /// built on the UI thread could therefore never decode a click on the menu
    /// the GTK thread put on screen: every item would map to a bare number,
    /// the application would match no arm, and the menu would open, close and
    /// do nothing. On Linux there is no `self.ids` field to hold such a copy,
    /// which is what keeps this from coming back.
    ///
    /// Windows only: `muda` on macOS wants the main thread, and on Linux it
    /// wants a display, neither of which a test thread has. The counter is the
    /// same design in all three backends, so checking it here checks it.
    #[test]
    #[cfg(target_os = "windows")]
    fn two_builds_of_the_same_menu_share_no_ids() {
        let items = vec![
            TrayMenuItem::new("show", "Show"),
            TrayMenuItem::separator(),
            TrayMenuItem::check("float", "Always on top", true),
            TrayMenuItem::submenu("Go to", vec![TrayMenuItem::new("stack", "Stack")]),
        ];
        let (_first_menu, first) = build_menu(&items).expect("the menu builds");
        let (_second_menu, second) = build_menu(&items).expect("the menu builds again");

        assert_eq!(first.len(), second.len(), "the same items, twice");
        assert!(!first.is_empty(), "a menu with no items proves nothing");

        for ((platform, ours), (other_platform, other_ours)) in first.iter().zip(&second) {
            assert_eq!(ours, other_ours, "the application ids are the caller's");
            assert_ne!(
                platform, other_platform,
                "muda reused a platform id for {ours}; if that is now true, the \
                 reason the Linux tray builds its menu on one thread is gone"
            );
        }
    }

    /// An icon whose buffer does not match its dimensions is rejected here,
    /// with the sizes in the message, rather than by the platform later.
    #[test]
    fn a_mis_sized_icon_is_refused_with_both_numbers() {
        let image = TrayIconImage {
            width: 8,
            height: 8,
            rgba: vec![0; 16],
        };
        let error = platform_icon(Some(&image)).unwrap_err();
        assert!(error.contains("256"), "{error}");
        assert!(error.contains("16"), "{error}");
    }
}
