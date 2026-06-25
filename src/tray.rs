use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

// Stable command ids returned by the hand-rolled right-click popup menu in app.rs.
// muda's automatic WM_COMMAND -> MenuEvent bridge (via SetWindowSubclass) turned out to
// be unreliable for tray context menus on at least one real machine - clicks in the menu
// never produced a MenuEvent at all, even though the menu displayed and dismissed
// correctly. Building the popup menu directly with TrackPopupMenu's TPM_RETURNCMD flag
// returns the chosen item synchronously with no event-posting/subclassing involved.
pub const CMD_SHOW_HIDE: u32 = 1;
pub const CMD_QUIT: u32 = 2;

pub struct SystemTray {
    pub tray_icon: TrayIcon,
}

// build.rs embeds resources/icon.ico into the exe via winres under resource id "1"
// (winres::set_icon defaults to name_id "1"), so we can load it straight from the
// running executable instead of drawing a placeholder icon or relying on the .ico
// file being present on disk next to the exe.
#[cfg(windows)]
fn load_tray_icon() -> Result<Icon, Box<dyn std::error::Error>> {
    // winres compiles the icon under the numeric resource id 1, not the string "1",
    // so this must be looked up by ordinal (from_resource), not from_resource_name.
    Ok(Icon::from_resource(1, None)?)
}

#[cfg(not(windows))]
fn load_tray_icon() -> Result<Icon, Box<dyn std::error::Error>> {
    let icon_bytes = include_bytes!("../resources/icon_512.png");
    let image = image::load_from_memory(icon_bytes)?.to_rgba8();
    let (width, height) = image.dimensions();
    Ok(Icon::from_rgba(image.into_raw(), width, height)?)
}

pub fn setup_tray() -> Result<SystemTray, Box<dyn std::error::Error>> {
    let icon = load_tray_icon()?;

    // No menu is attached here - the right-click context menu is built and shown
    // manually (see app::show_tray_context_menu) in response to the tray icon's own
    // right-click event, bypassing tray-icon's built-in menu display entirely.
    let tray_icon = TrayIconBuilder::new()
        .with_menu_on_left_click(false) // reserve right-click for our own context menu; left double-click toggles the window
        .with_menu_on_right_click(false)
        .with_tooltip("HueMIDIty")
        .with_icon(icon)
        .build()?;

    Ok(SystemTray { tray_icon })
}
