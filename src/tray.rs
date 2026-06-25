use muda::{Menu, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

pub struct SystemTray {
    pub tray_icon: TrayIcon,
    pub show_hide_id: String,
    pub quit_id: String,
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
    let menu = Menu::new();
    
    let show_hide = MenuItem::new("Show / Hide Window", true, None);
    let quit = MenuItem::new("Quit HueMIDIty", true, None);
    
    let show_hide_id = show_hide.id().clone().0;
    let quit_id = quit.id().clone().0;
    
    menu.append(&show_hide)?;
    menu.append(&PredefinedMenuItem::separator())?;
    menu.append(&quit)?;
    
    let tray_icon = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_menu_on_left_click(false) // reserve the context menu for right-click; left double-click toggles the window
        .with_tooltip("HueMIDIty")
        .with_icon(icon)
        .build()?;
        
    Ok(SystemTray {
        tray_icon,
        show_hide_id,
        quit_id,
    })
}
