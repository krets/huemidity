use muda::{Menu, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

pub struct SystemTray {
    pub tray_icon: TrayIcon,
    pub show_hide_id: String,
    pub quit_id: String,
}

pub fn setup_tray() -> Result<SystemTray, Box<dyn std::error::Error>> {
    let width = 16;
    let height = 16;
    let mut rgba = vec![0u8; width * height * 4];
    
    // Draw a purple circle with an inner yellow dot representing a light
    for y in 0..height {
        for x in 0..width {
            let dx = x as f32 - 7.5;
            let dy = y as f32 - 7.5;
            let dist = (dx * dx + dy * dy).sqrt();
            let idx = (y * width + x) * 4;
            
            if dist <= 3.0 {
                // Yellow core
                rgba[idx] = 253;     // R
                rgba[idx + 1] = 224; // G
                rgba[idx + 2] = 71;  // B
                rgba[idx + 3] = 255; // A
            } else if dist <= 6.5 {
                // Purple outer ring
                rgba[idx] = 168;     // R
                rgba[idx + 1] = 85;  // G
                rgba[idx + 2] = 247; // B
                rgba[idx + 3] = 255; // A
            } else {
                // Transparent background
                rgba[idx] = 0;
                rgba[idx + 1] = 0;
                rgba[idx + 2] = 0;
                rgba[idx + 3] = 0;
            }
        }
    }
    
    let icon = Icon::from_rgba(rgba, width as u32, height as u32)?;
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
        .with_tooltip("HueMIDIty")
        .with_icon(icon)
        .build()?;
        
    Ok(SystemTray {
        tray_icon,
        show_hide_id,
        quit_id,
    })
}
