use eframe::egui;
use crate::config::{AppConfig, DashboardItem, Mapping};
use crate::hue::BridgeConnectionState;
use crate::hue::types::{Light, Group, Scene, Capability};
use crate::midi::{MidiEvent, get_midi_ports};

// egui ships without a bold font variant, so fake bold by overdrawing the glyphs
// with a tiny horizontal offset a few times to thicken the strokes.
fn draw_bold_text(ui: &mut egui::Ui, text: &str, size: f32, color: egui::Color32) {
    let font_id = egui::FontId::proportional(size);
    let galley = ui.painter().layout_no_wrap(text.to_string(), font_id, color);
    let extra = 0.8;
    let desired_size = galley.size() + egui::vec2(extra, 0.0);
    let (rect, _response) = ui.allocate_exact_size(desired_size, egui::Sense::hover());
    let painter = ui.painter();
    for dx in [0.0, extra * 0.5, extra] {
        painter.galley(rect.min + egui::vec2(dx, 0.0), galley.clone(), color);
    }
}

pub fn setup_custom_theme(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    
    // Background and frame colors
    visuals.panel_fill = egui::Color32::from_rgb(11, 12, 18); // Darker background with same hue as widgets
    visuals.window_fill = egui::Color32::from_rgb(22, 25, 37); // Glassmorphism container
    
    // Widget styles
    visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(28, 32, 48);
    visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(200, 200, 220));
    visuals.widgets.inactive.corner_radius = egui::CornerRadius::same(6);
    
    visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(38, 44, 66);
    visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(255, 255, 255));
    visuals.widgets.hovered.corner_radius = egui::CornerRadius::same(6);
    
    visuals.widgets.active.bg_fill = egui::Color32::from_rgb(168, 85, 247); // Purple accent
    visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(255, 255, 255));
    visuals.widgets.active.corner_radius = egui::CornerRadius::same(6);
    
    // Slider colors
    visuals.selection.bg_fill = egui::Color32::from_rgb(168, 85, 247);
    
    ctx.set_visuals(visuals);
}

fn toggle_ui(ui: &mut egui::Ui, on: &mut bool) -> egui::Response {
    let desired_size = ui.spacing().interact_size.y * egui::vec2(2.0, 1.0);
    let (rect, mut response) = ui.allocate_exact_size(desired_size, egui::Sense::click());
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Checkbox, true, *on, ""));

    if ui.is_rect_visible(rect) {
        let how_on = ui.ctx().animate_bool(response.id, *on);
        let visuals = ui.style().interact_selectable(&response, *on);
        let rect = rect.expand(visuals.expansion);
        let radius = 0.5 * rect.height();
        
        let bg_fill = if *on {
            egui::Color32::from_rgb(168, 85, 247)
        } else {
            egui::Color32::from_rgb(55, 65, 81)
        };
        ui.painter().rect_filled(rect, radius, bg_fill);
        
        let circle_x = egui::lerp((rect.left() + radius)..=(rect.right() - radius), how_on);
        let center = egui::pos2(circle_x, rect.center().y);
        ui.painter().circle(
            center,
            0.75 * radius,
            egui::Color32::WHITE,
            visuals.fg_stroke,
        );
    }
    response
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (f32, f32, f32) {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0).rem_euclid(2.0) - 1.0).abs());
    let m = v - c;

    let (r_prime, g_prime, b_prime) = if h >= 0.0 && h < 60.0 {
        (c, x, 0.0)
    } else if h >= 60.0 && h < 120.0 {
        (x, c, 0.0)
    } else if h >= 120.0 && h < 180.0 {
        (0.0, c, x)
    } else if h >= 180.0 && h < 240.0 {
        (0.0, x, c)
    } else if h >= 240.0 && h < 300.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };

    (r_prime + m, g_prime + m, b_prime + m)
}

fn rgb_to_hsv(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let c_max = r.max(g).max(b);
    let c_min = r.min(g).min(b);
    let delta = c_max - c_min;

    let h = if delta == 0.0 {
        0.0
    } else if c_max == r {
        60.0 * (((g - b) / delta).rem_euclid(6.0))
    } else if c_max == g {
        60.0 * (((b - r) / delta) + 2.0)
    } else {
        60.0 * (((r - g) / delta) + 4.0)
    };

    let s = if c_max == 0.0 { 0.0 } else { delta / c_max };
    let v = c_max;

    (h, s, v)
}


use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::mpsc::{UnboundedSender as Sender, UnboundedReceiver as Receiver};

#[derive(Clone, Debug)]
pub enum GuiMessage {
    MidiActivity(MidiEvent),
    MidiStatus(String),
    HueConnectionState(BridgeConnectionState),
    DevicesRefreshed {
        lights: HashMap<String, Light>,
        groups: HashMap<String, Group>,
        scenes: HashMap<String, Scene>,
        requested_at: Instant,
    },
    Error(String),
}

#[derive(Clone, Debug)]
pub enum BgMessage {
    ConnectManual(String),
    StartAutoDiscovery,
    CancelLink,
    ForgetBridge,
    RefreshDevices,
    SetLightState {
        light_id: String,
        state: serde_json::Value,
    },
    SetGroupAction {
        group_id: String,
        action: serde_json::Value,
    },
    // Sent by a background task once a light/group PUT has succeeded and settled.
    // Only honored if `seq` still matches the latest dispatched request for that id.
    CompletedLightPut {
        light_id: String,
        seq: u64,
    },
    CompletedGroupPut {
        group_id: String,
        seq: u64,
    },
    RecallScene {
        group_id: String,
        scene_id: String,
    },
    ChangeMidiPort(String),
    MidiInputReceived(MidiEvent),
    UpdateConfig(AppConfig),
}

#[derive(Clone, PartialEq, Eq)]
pub enum Tab {
    Dashboard,
    MidiMapping,
}

pub struct LogEntry {
    pub time: String,
    pub event_key: String,
    pub value: u8,
}

// Modal structures
pub struct MappingCreatorState {
    pub is_open: bool,
    pub event_key: String,
    pub target_type: String, // "light", "group", "scene"
    pub target_id: String,
    pub action: String,
    pub invert: bool,
    pub auto_on: bool,
}

pub struct AddWidgetsState {
    pub is_open: bool,
    pub search_query: String,
    pub selected_lights: HashSet<String>,
    pub selected_groups: HashSet<String>,
}

pub struct SettingsState {
    pub is_open: bool,
}

#[derive(Clone)]
pub enum ConfirmAction {
    ForgetBridge,
    RemoveDashboardItem(usize),
    RemoveMapping(String),
}

pub struct ConfirmDialogState {
    pub is_open: bool,
    pub title: String,
    pub message: String,
    pub action: Option<ConfirmAction>,
}

pub struct HueMIDItyApp {
    // Communication channels
    pub gui_rx: Receiver<GuiMessage>,
    pub bg_tx: Sender<BgMessage>,

    // App state
    pub config: AppConfig,
    pub connection_state: BridgeConnectionState,
    pub active_tab: Tab,

    // Devices cache
    pub lights: HashMap<String, Light>,
    pub groups: HashMap<String, Group>,
    pub scenes: HashMap<String, Scene>,

    // MIDI details
    pub midi_ports: Vec<String>,
    pub midi_log: Vec<LogEntry>,
    pub midi_status: String, // "Live Input: Active", "Connecting...", etc.

    // Modal state
    pub mapping_creator: MappingCreatorState,
    pub add_widgets: AddWidgetsState,
    pub settings: SettingsState,
    pub confirm_dialog: ConfirmDialogState,

    // Cached textures
    pub app_icon_texture: Option<egui::TextureHandle>,

    // Tray Menu IDs
    pub tray_show_hide_id: String,
    pub tray_quit_id: String,

    // Timing and UI effects
    pub last_port_refresh: Instant,
    pub manual_ip_input: String,

    // Timestamp of the last local brightness edit per light/group. A device refresh
    // that was requested before this timestamp is stale for that id and must be ignored,
    // otherwise an in-flight refresh can land after the user moves on and snap the slider back.
    pub light_last_local_edit: HashMap<String, Instant>,
    pub group_last_local_edit: HashMap<String, Instant>,

    // Index of the dashboard widget currently being dragged via its drag handle, if any.
    pub dashboard_drag: Option<usize>,

    // Tray/window lifecycle: closing the window hides it instead of exiting; the app
    // only truly quits via the tray menu's Quit item or the Settings "Quit App" button.
    pub window_visible: bool,
    // If the tray icon failed to initialize there would be no way to bring a hidden
    // window back, so closing the window must quit normally in that case.
    pub tray_available: bool,
}

impl HueMIDItyApp {
    pub fn new(
        config: AppConfig,
        gui_rx: Receiver<GuiMessage>,
        bg_tx: Sender<BgMessage>,
        tray_show_hide_id: String,
        tray_quit_id: String,
        tray_available: bool,
    ) -> Self {
        // Auto-refresh devices on startup if connected
        let connection_state = if !config.bridge_ip.is_empty() && !config.bridge_username.is_empty() {
            bg_tx.send(BgMessage::RefreshDevices).ok();
            BridgeConnectionState::Connected {
                ip: config.bridge_ip.clone(),
                username: config.bridge_username.clone(),
            }
        } else if !config.bridge_ip.is_empty() {
            BridgeConnectionState::NeedsLink {
                ip: config.bridge_ip.clone(),
                countdown: 120,
            }
        } else {
            bg_tx.send(BgMessage::StartAutoDiscovery).ok();
            BridgeConnectionState::Searching
        };

        if !config.selected_device.is_empty() {
            bg_tx.send(BgMessage::ChangeMidiPort(config.selected_device.clone())).ok();
        }

        Self {
            gui_rx,
            bg_tx,
            manual_ip_input: config.bridge_ip.clone(),
            config,
            connection_state,
            active_tab: Tab::Dashboard,
            lights: HashMap::new(),
            groups: HashMap::new(),
            scenes: HashMap::new(),
            midi_ports: get_midi_ports(),
            midi_log: Vec::new(),
            midi_status: "Disconnected".to_string(),
            mapping_creator: MappingCreatorState {
                is_open: false,
                event_key: String::new(),
                target_type: "light".to_string(),
                target_id: String::new(),
                action: "Brightness".to_string(),
                invert: false,
                auto_on: true,
            },
            add_widgets: AddWidgetsState {
                is_open: false,
                search_query: String::new(),
                selected_lights: HashSet::new(),
                selected_groups: HashSet::new(),
            },
            settings: SettingsState {
                is_open: false,
            },
            confirm_dialog: ConfirmDialogState {
                is_open: false,
                title: String::new(),
                message: String::new(),
                action: None,
            },
            app_icon_texture: None,
            tray_show_hide_id,
            tray_quit_id,
            last_port_refresh: Instant::now(),
            light_last_local_edit: HashMap::new(),
            group_last_local_edit: HashMap::new(),
            dashboard_drag: None,
            window_visible: true,
            tray_available,
        }
    }

    fn check_channels(&mut self, ctx: &egui::Context) {
        let mut got_msg = false;
        while let Ok(msg) = self.gui_rx.try_recv() {
            got_msg = true;
            match msg {
                GuiMessage::MidiActivity(event) => {
                    // Update log (keep last 10 entries)
                    let now = chrono::Local::now().format("%H:%M:%S").to_string();
                    self.midi_log.insert(0, LogEntry {
                        time: now,
                        event_key: event.event_key.clone(),
                        value: event.value,
                    });
                    if self.midi_log.len() > 10 {
                        self.midi_log.truncate(10);
                    }
                    self.midi_status = "Live Input: Active".to_string();
                }
                GuiMessage::MidiStatus(status) => {
                    self.midi_status = status;
                }
                GuiMessage::HueConnectionState(state) => {
                    self.connection_state = state.clone();
                    match &state {
                        BridgeConnectionState::Connected { ip, username } => {
                            self.config.bridge_ip = ip.clone();
                            self.config.bridge_username = username.clone();
                            self.config.save().ok();
                            self.bg_tx.send(BgMessage::UpdateConfig(self.config.clone())).ok();
                            self.bg_tx.send(BgMessage::RefreshDevices).ok();
                        }
                        BridgeConnectionState::Idle => {
                            self.lights.clear();
                            self.groups.clear();
                            self.scenes.clear();
                        }
                        _ => {}
                    }
                }
                GuiMessage::DevicesRefreshed { mut lights, mut groups, scenes, requested_at } => {
                    if !self.add_widgets.is_open {
                        // Hue bulbs apply brightness over a short physical transition (~400ms), so even
                        // a refresh requested *after* our last edit can still report an in-between value.
                        // Ignore refreshes for a light/group until a settle window has passed since the
                        // last local edit, and also reject any refresh that was requested before that
                        // edit (an out-of-order/stale response for a since-superseded value).
                        const SETTLE_WINDOW: Duration = Duration::from_millis(700);
                        let now = Instant::now();
                        for (id, edited_at) in &self.light_last_local_edit {
                            if *edited_at > requested_at || now.duration_since(*edited_at) < SETTLE_WINDOW {
                                if let (Some(old), Some(new)) = (self.lights.get(id), lights.get_mut(id)) {
                                    new.state.bri = old.state.bri;
                                    new.state.on = old.state.on;
                                }
                            }
                        }
                        for (id, edited_at) in &self.group_last_local_edit {
                            if *edited_at > requested_at || now.duration_since(*edited_at) < SETTLE_WINDOW {
                                if let (Some(old), Some(new)) = (self.groups.get(id), groups.get_mut(id)) {
                                    new.action.bri = old.action.bri;
                                    new.action.on = old.action.on;
                                }
                            }
                        }

                        self.lights = lights;
                        self.groups = groups;
                        self.scenes = scenes;
                    }
                }
                GuiMessage::Error(err_msg) => {
                    self.midi_status = format!("Conflict: {}", err_msg);
                }
            }
        }
        if got_msg {
            ctx.request_repaint();
        }
    }

    fn ensure_app_icon(&mut self, ctx: &egui::Context) -> Option<egui::TextureHandle> {
        if self.app_icon_texture.is_none() {
            let icon_bytes = include_bytes!("../resources/icon_512.png");
            if let Ok(img) = image::load_from_memory_with_format(icon_bytes, image::ImageFormat::Png) {
                let img = img.to_rgba8();
                let size = [img.width() as usize, img.height() as usize];
                let pixels = img.into_raw();
                let color_image = egui::ColorImage::from_rgba_unmultiplied(size, &pixels);
                self.app_icon_texture = Some(ctx.load_texture("app_icon", color_image, egui::TextureOptions::default()));
            }
        }
        self.app_icon_texture.clone()
    }

    pub fn draw_onboarding(&mut self, ctx: &egui::Context) {
        let icon_texture = self.ensure_app_icon(ctx);
        let body_text_color = egui::Color32::from_rgb(214, 214, 222);
        let accent_color = egui::Color32::from_rgb(168, 85, 247);

        let panel_frame = egui::Frame::default().fill(egui::Color32::from_rgb(12, 12, 14));
        egui::CentralPanel::default().frame(panel_frame).show(ctx, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(40.0);

                egui::Frame::default()
                    .fill(egui::Color32::from_rgb(26, 26, 30))
                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(46, 46, 52)))
                    .corner_radius(egui::CornerRadius::same(14))
                    .inner_margin(egui::Margin::symmetric(36, 36))
                    .show(ui, |ui| {
                        ui.set_width(360.0);
                        ui.style_mut().spacing.button_padding = egui::vec2(18.0, 10.0);

                        ui.vertical_centered(|ui| {
                            if let Some(tex) = &icon_texture {
                                ui.add(egui::Image::new(tex).max_width(72.0).max_height(72.0));
                                ui.add_space(14.0);
                            }

                            draw_bold_text(ui, "HueMIDIty", 30.0, accent_color);
                            ui.add_space(8.0);
                            ui.label(
                                egui::RichText::new("Bind physical MIDI inputs to Philips Hue lighting controls.")
                                    .color(body_text_color),
                            );
                            ui.add_space(32.0);

                            match &self.connection_state {
                                BridgeConnectionState::Searching => {
                                    ui.add(egui::Spinner::new().size(32.0));
                                    ui.add_space(20.0);
                                    ui.label(
                                        egui::RichText::new("Searching for Philips Hue Bridge on your local network...")
                                            .color(body_text_color),
                                    );
                                    ui.add_space(30.0);

                                    if ui.button("Enter IP Manually").clicked() {
                                        self.connection_state = BridgeConnectionState::Idle;
                                    }
                                }
                                BridgeConnectionState::Idle => {
                                    ui.label(
                                        egui::RichText::new("Enter the IP address of your Philips Hue Bridge:")
                                            .color(body_text_color),
                                    );
                                    ui.add_space(10.0);

                                    ui.horizontal(|ui| {
                                        ui.add(
                                            egui::TextEdit::singleline(&mut self.manual_ip_input)
                                                .desired_width(220.0)
                                                .hint_text("e.g. 192.168.1.10"),
                                        );
                                    });

                                    ui.add_space(20.0);
                                    ui.vertical_centered_justified(|ui| {
                                        let connect_btn = egui::Button::new(
                                            egui::RichText::new("Connect").color(egui::Color32::WHITE).strong(),
                                        ).fill(accent_color);
                                        if ui.add(connect_btn).clicked() {
                                            if !self.manual_ip_input.is_empty() {
                                                self.bg_tx.send(BgMessage::ConnectManual(self.manual_ip_input.trim().to_string())).ok();
                                            }
                                        }

                                        ui.add_space(8.0);

                                        if ui.button("Auto-Discover Again").clicked() {
                                            self.bg_tx.send(BgMessage::StartAutoDiscovery).ok();
                                            self.connection_state = BridgeConnectionState::Searching;
                                        }
                                    });
                                }
                                BridgeConnectionState::NeedsLink { ip, countdown } => {
                                    // Display pulsing lock button message
                                    ui.heading(
                                        egui::RichText::new("Authentication Required")
                                            .color(egui::Color32::from_rgb(253, 224, 71))
                                            .strong()
                                    );
                                    ui.add_space(15.0);
                                    ui.label(
                                        egui::RichText::new("Press the physical Link Button on your Hue Bridge now to connect.")
                                            .size(16.0)
                                            .color(body_text_color),
                                    );
                                    ui.add_space(20.0);

                                    // ProgressBar countdown
                                    let progress = *countdown as f32 / 120.0;
                                    ui.add(egui::ProgressBar::new(progress).text(format!("{} seconds remaining", countdown)));
                                    ui.add_space(15.0);
                                    ui.label(
                                        egui::RichText::new(format!("Bridge IP: {}", ip)).color(body_text_color),
                                    );

                                    ui.add_space(20.0);
                                    if ui.button("Cancel").clicked() {
                                        self.bg_tx.send(BgMessage::CancelLink).ok();
                                        self.connection_state = BridgeConnectionState::Idle;
                                    }
                                }
                                BridgeConnectionState::Error(err) => {
                                    ui.colored_label(egui::Color32::from_rgb(239, 68, 68), "Connection Error");
                                    ui.label(
                                        egui::RichText::new(format!("Unable to connect to the Hue Bridge: {}", err))
                                            .color(body_text_color),
                                    );
                                    ui.add_space(20.0);

                                    if ui.button("Retry Auto-Discovery").clicked() {
                                        self.bg_tx.send(BgMessage::StartAutoDiscovery).ok();
                                        self.connection_state = BridgeConnectionState::Searching;
                                    }
                                    if ui.button("Enter IP Manually").clicked() {
                                        self.connection_state = BridgeConnectionState::Idle;
                                    }
                                }
                                _ => {}
                            }
                        });
                    });
            });
            });
        });
    }

    pub fn draw_main_app(&mut self, ctx: &egui::Context) {
        // Refresh midi ports every 5 seconds
        if self.last_port_refresh.elapsed().as_secs() > 5 {
            self.midi_ports = get_midi_ports();
            self.last_port_refresh = Instant::now();
        }

        let any_modal_open = self.add_widgets.is_open || self.mapping_creator.is_open || self.settings.is_open || self.confirm_dialog.is_open;

        // Top panel header
        egui::TopBottomPanel::top("top_header").show(ctx, |ui| {
            ui.add_enabled_ui(!any_modal_open, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("HueMIDIty");
                    ui.add_space(20.0);

                    // Tab buttons
                    ui.selectable_value(&mut self.active_tab, Tab::Dashboard, "📋 Dashboard");
                    ui.selectable_value(&mut self.active_tab, Tab::MidiMapping, "🎹 MIDI Mapping");
                    if ui.button("⚙ Settings").clicked() {
                        self.settings.is_open = true;
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Refresh button
                        if ui.button("🔄 Refresh").clicked() {
                            self.bg_tx.send(BgMessage::RefreshDevices).ok();
                        }

                        // Green connection dot
                        ui.horizontal(|ui| {
                            let color = match &self.connection_state {
                                BridgeConnectionState::Connected { .. } => egui::Color32::from_rgb(34, 197, 94), // Green
                                _ => egui::Color32::from_rgb(239, 68, 68), // Red
                            };
                            let (rect, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
                            ui.painter().circle_filled(rect.center(), 5.0, color);
                            ui.label("Connected");
                        });
                    });
                });
            });

            // Dim the header in place when a modal is open, without blocking the modal's input.
            if any_modal_open {
                ui.painter().rect_filled(ui.max_rect(), 0.0, egui::Color32::from_black_alpha(150));
            }
        });

        // Content
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_enabled_ui(!any_modal_open, |ui| {
                match self.active_tab {
                    Tab::Dashboard => self.draw_dashboard_tab(ui),
                    Tab::MidiMapping => self.draw_midi_mapping_tab(ui),
                }
            });

            // Dim the content area in place when a modal is open, without blocking the modal's input.
            if any_modal_open {
                ui.painter().rect_filled(ui.max_rect(), 0.0, egui::Color32::from_black_alpha(150));
            }
        });

        // Modals
        self.draw_mapping_modal(ctx);
        self.draw_widget_modal(ctx);
        self.draw_settings_modal(ctx);
        self.draw_confirm_dialog(ctx);
    }

    fn request_confirm(&mut self, title: &str, message: &str, action: ConfirmAction) {
        self.confirm_dialog.is_open = true;
        self.confirm_dialog.title = title.to_string();
        self.confirm_dialog.message = message.to_string();
        self.confirm_dialog.action = Some(action);
    }

    fn draw_confirm_dialog(&mut self, ctx: &egui::Context) {
        if !self.confirm_dialog.is_open {
            return;
        }

        // Full-screen dimming overlay, above any other modal, to make this dialog block everything.
        egui::Area::new(egui::Id::new("confirm_dialog_overlay"))
            .order(egui::Order::Foreground)
            .fixed_pos(egui::Pos2::ZERO)
            .show(ctx, |ui| {
                let screen_rect = ctx.content_rect();
                ui.allocate_rect(screen_rect, egui::Sense::click());
                ui.painter().rect_filled(screen_rect, 0.0, egui::Color32::from_black_alpha(180));
            });

        let mut open = true;
        egui::Window::new(&self.confirm_dialog.title)
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .order(egui::Order::Foreground)
            .default_width(360.0)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label(&self.confirm_dialog.message);
                ui.add_space(16.0);

                let full_width = ui.available_width();
                ui.allocate_ui_with_layout(egui::vec2(full_width, 28.0), egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.style_mut().spacing.button_padding = egui::vec2(12.0, 6.0);
                    ui.style_mut().spacing.item_spacing.y = 0.0;

                    if ui.button("Cancel").clicked() {
                        self.confirm_dialog.is_open = false;
                        self.confirm_dialog.action = None;
                    }

                    let danger_color = egui::Color32::from_rgb(239, 68, 68);
                    let confirm_btn = egui::Button::new(egui::RichText::new("Confirm").color(egui::Color32::WHITE))
                        .fill(egui::Color32::from_rgb(127, 29, 29))
                        .stroke(egui::Stroke::new(1.0, danger_color));
                    if ui.add(confirm_btn).clicked() {
                        if let Some(action) = self.confirm_dialog.action.take() {
                            match action {
                                ConfirmAction::ForgetBridge => {
                                    self.bg_tx.send(BgMessage::ForgetBridge).ok();
                                    self.config.bridge_ip.clear();
                                    self.config.bridge_username.clear();
                                    self.config.save().ok();
                                    self.bg_tx.send(BgMessage::UpdateConfig(self.config.clone())).ok();
                                    self.connection_state = BridgeConnectionState::Idle;
                                }
                                ConfirmAction::RemoveDashboardItem(idx) => {
                                    if idx < self.config.dashboard_layout.len() {
                                        self.config.dashboard_layout.remove(idx);
                                        self.config.save().ok();
                                        self.bg_tx.send(BgMessage::UpdateConfig(self.config.clone())).ok();
                                    }
                                }
                                ConfirmAction::RemoveMapping(key) => {
                                    let device = self.config.selected_device.clone();
                                    if let Some(maps) = self.config.mappings.get_mut(&device) {
                                        maps.remove(&key);
                                        self.config.save().ok();
                                        self.bg_tx.send(BgMessage::UpdateConfig(self.config.clone())).ok();
                                    }
                                }
                            }
                        }
                        self.confirm_dialog.is_open = false;
                    }
                });
            });

        if !open {
            self.confirm_dialog.is_open = false;
            self.confirm_dialog.action = None;
        }
    }

    fn draw_dashboard_tab(&mut self, ui: &mut egui::Ui) {
        if self.config.dashboard_layout.is_empty() {
            ui.vertical_centered(|ui| {
                ui.add_space(100.0);
                ui.label(
                    egui::RichText::new("Your dashboard is empty.")
                        .size(18.0)
                        .weak()
                );
                ui.label("Add light and group widgets to build your custom layout.");
                ui.add_space(20.0);
                if ui.button("+ Add Widgets").clicked() {
                    self.add_widgets.is_open = true;
                    self.add_widgets.selected_lights.clear();
                    self.add_widgets.selected_groups.clear();
                }
            });
            return;
        }

        // Safety net: if the drag ended outside any widget (e.g. released off-window),
        // the handle's own drag_stopped() event never fires, so clear it here instead.
        if self.dashboard_drag.is_some() && !ui.input(|i| i.pointer.button_down(egui::PointerButton::Primary)) {
            self.dashboard_drag = None;
        }

        // Scrollable widgets grid
        egui::ScrollArea::vertical().show(ui, |ui| {
            let width = ui.available_width() - 8.0; // Subtract 8.0 to account for scrollbar
            let layout_items = self.config.dashboard_layout.clone();
            let num_items = layout_items.len();
            let columns = if num_items == 0 {
                1
            } else {
                let max_cols = (width / 240.0).floor().max(1.0) as usize;
                max_cols.min(num_items)
            };
            let spacing_x = 12.0;
            let widget_width = (width - (columns - 1) as f32 * spacing_x) / columns as f32;
            
            // Simple grid layout
            egui::Grid::new("dashboard_grid")
                .spacing(egui::vec2(spacing_x, 12.0))
                .show(ui, |ui| {
                    let mut swap_indices = None;
                    
                    for (idx, item) in layout_items.iter().enumerate() {
                        let is_being_dragged = self.dashboard_drag == Some(idx);
                        let border_color = if is_being_dragged {
                            egui::Color32::from_rgb(168, 85, 247)
                        } else {
                            egui::Color32::from_rgb(44, 48, 68)
                        };
                        let card_response = egui::Frame::window(ui.style())
                            .stroke(egui::Stroke::new(if is_being_dragged { 2.0 } else { 1.0 }, border_color))
                            .shadow(egui::Shadow::NONE)
                            .inner_margin(egui::Margin::symmetric(16, 12))
                            .show(ui, |ui| {
                                let content_width = widget_width - 34.0;
                                ui.set_min_width(content_width);
                                ui.set_max_width(content_width);
                                ui.set_min_height(110.0);
                                
                                ui.vertical(|ui| {
                                    // Header of card
                                    ui.horizontal(|ui| {
                                        let drag_handle = ui.add(
                                            egui::Label::new(
                                                egui::RichText::new("⠿").color(if is_being_dragged {
                                                    egui::Color32::from_rgb(168, 85, 247)
                                                } else {
                                                    egui::Color32::from_rgb(140, 140, 160)
                                                })
                                            )
                                            .sense(egui::Sense::drag())
                                        );

                                        if drag_handle.hovered() || drag_handle.dragged() {
                                            ui.ctx().set_cursor_icon(if drag_handle.dragged() {
                                                egui::CursorIcon::Grabbing
                                            } else {
                                                egui::CursorIcon::Grab
                                            });
                                        }
                                        if drag_handle.drag_started() {
                                            self.dashboard_drag = Some(idx);
                                        }
                                        if drag_handle.drag_stopped() {
                                            self.dashboard_drag = None;
                                        }

                                        let name = if item.r#type == "light" {
                                            self.lights.get(&item.id).map(|l| l.name.clone()).unwrap_or_else(|| format!("Light {}", item.id))
                                        } else {
                                            self.groups.get(&item.id).map(|g| g.name.clone()).unwrap_or_else(|| format!("Group {}", item.id))
                                        };

                                        ui.heading(egui::RichText::new(&name).size(14.0).strong());
                                        
                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                            ui.scope(|ui| {
                                                ui.style_mut().visuals.widgets.inactive.bg_fill = egui::Color32::TRANSPARENT;
                                                ui.style_mut().visuals.widgets.inactive.weak_bg_fill = egui::Color32::TRANSPARENT;
                                                ui.style_mut().visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(140, 140, 160));
                                                ui.style_mut().visuals.widgets.inactive.bg_stroke = egui::Stroke::NONE;
                                                if ui.button("×").clicked() {
                                                    self.request_confirm(
                                                        "Remove Widget?",
                                                        &format!("Remove \"{}\" from the dashboard? You can add it back later from Add Widgets.", name),
                                                        ConfirmAction::RemoveDashboardItem(idx),
                                                    );
                                                }
                                            });
                                        });
                                    });

                                    ui.add_space(8.0);

                                    // Display controls depending on light or group
                                    if item.r#type == "light" {
                                        if let Some(light) = self.lights.get_mut(&item.id) {
                                            ui.horizontal(|ui| {
                                                // Toggle switch on the left
                                                let mut is_on = light.state.on.unwrap_or(false);
                                                if toggle_ui(ui, &mut is_on).changed() {
                                                    light.state.on = Some(is_on);
                                                    self.bg_tx.send(BgMessage::SetLightState {
                                                        light_id: item.id.clone(),
                                                        state: serde_json::json!({ "on": is_on }),
                                                    }).ok();
                                                }

                                                // Colorswatch right-aligned
                                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                    if light.capabilities().contains(&Capability::Color) {
                                                        let h = light.state.hue.unwrap_or(0) as f32 / 65535.0 * 360.0;
                                                        let s = light.state.sat.unwrap_or(0) as f32 / 254.0;
                                                        let v = light.state.bri.unwrap_or(254) as f32 / 254.0;
                                                        let (r_val, g_val, b_val) = hsv_to_rgb(h, s, v);
                                                        let mut color = egui::Color32::from_rgb((r_val * 255.0) as u8, (g_val * 255.0) as u8, (b_val * 255.0) as u8);

                                                        if ui.color_edit_button_srgba(&mut color).changed() {
                                                            let (new_h, new_s, new_v) = rgb_to_hsv(color.r() as f32 / 255.0, color.g() as f32 / 255.0, color.b() as f32 / 255.0);
                                                            let hue = (new_h / 360.0 * 65535.0) as u16;
                                                            let sat = (new_s * 254.0) as u8;
                                                            let bri = (new_v * 254.0) as u8;

                                                            light.state.hue = Some(hue);
                                                            light.state.sat = Some(sat);
                                                            light.state.bri = Some(bri);
                                                            light.state.on = Some(bri > 0);

                                                            let mut state = serde_json::json!({
                                                                "on": bri > 0,
                                                                "hue": hue,
                                                                "sat": sat,
                                                                "bri": bri
                                                            });
                                                            if bri == 0 {
                                                                state = serde_json::json!({ "on": false });
                                                            }

                                                            self.bg_tx.send(BgMessage::SetLightState {
                                                                light_id: item.id.clone(),
                                                                state,
                                                            }).ok();
                                                        }
                                                    }
                                                });
                                            });

                                            ui.add_space(8.0);

                                            // Full-width Brightness Slider without name/value text (0 turns OFF)
                                            if light.state.bri.is_some() {
                                                let mut bri = light.state.bri.unwrap_or(0);
                                                ui.spacing_mut().slider_width = ui.available_width();
                                                let slider_res = ui.add_sized(
                                                    egui::vec2(ui.available_width(), 16.0),
                                                    egui::Slider::new(&mut bri, 0..=254).show_value(false),
                                                );
                                                if slider_res.changed() {
                                                    let is_on = bri > 0;
                                                    light.state.bri = Some(bri);
                                                    light.state.on = Some(is_on);
                                                    self.light_last_local_edit.insert(item.id.clone(), Instant::now());

                                                    let mut state = serde_json::json!({ "bri": bri, "on": is_on });
                                                    if bri == 0 {
                                                        state = serde_json::json!({ "on": false });
                                                    }
                                                    self.bg_tx.send(BgMessage::SetLightState {
                                                        light_id: item.id.clone(),
                                                        state,
                                                    }).ok();
                                                }
                                            }
                                        } else {
                                            ui.horizontal(|ui| {
                                                ui.colored_label(egui::Color32::from_rgb(120, 120, 120), "Offline");
                                            });
                                        }
                                    } else if item.r#type == "group" {
                                        if let Some(group) = self.groups.get_mut(&item.id) {
                                            ui.horizontal(|ui| {
                                                // Toggle switch on the left
                                                let mut is_on = group.action.on.unwrap_or(false);
                                                if toggle_ui(ui, &mut is_on).changed() {
                                                    group.action.on = Some(is_on);
                                                    self.bg_tx.send(BgMessage::SetGroupAction {
                                                        group_id: item.id.clone(),
                                                        action: serde_json::json!({ "on": is_on }),
                                                    }).ok();
                                                }

                                                // Colorswatch right-aligned
                                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                    let mut group_supports_color = false;
                                                    for light_id in &group.lights {
                                                        if let Some(l) = self.lights.get(light_id) {
                                                            if l.capabilities().contains(&Capability::Color) {
                                                                group_supports_color = true;
                                                                break;
                                                            }
                                                        }
                                                    }

                                                    if group_supports_color {
                                                        let h = group.action.hue.unwrap_or(0) as f32 / 65535.0 * 360.0;
                                                        let s = group.action.sat.unwrap_or(0) as f32 / 254.0;
                                                        let v = group.action.bri.unwrap_or(254) as f32 / 254.0;
                                                        let (r_val, g_val, b_val) = hsv_to_rgb(h, s, v);
                                                        let mut color = egui::Color32::from_rgb((r_val * 255.0) as u8, (g_val * 255.0) as u8, (b_val * 255.0) as u8);

                                                        if ui.color_edit_button_srgba(&mut color).changed() {
                                                            let (new_h, new_s, new_v) = rgb_to_hsv(color.r() as f32 / 255.0, color.g() as f32 / 255.0, color.b() as f32 / 255.0);
                                                            let hue = (new_h / 360.0 * 65535.0) as u16;
                                                            let sat = (new_s * 254.0) as u8;
                                                            let bri = (new_v * 254.0) as u8;

                                                            group.action.hue = Some(hue);
                                                            group.action.sat = Some(sat);
                                                            group.action.bri = Some(bri);
                                                            group.action.on = Some(bri > 0);

                                                            let mut action = serde_json::json!({
                                                                "on": bri > 0,
                                                                "hue": hue,
                                                                "sat": sat,
                                                                "bri": bri
                                                            });
                                                            if bri == 0 {
                                                                action = serde_json::json!({ "on": false });
                                                            }

                                                            self.bg_tx.send(BgMessage::SetGroupAction {
                                                                group_id: item.id.clone(),
                                                                action,
                                                            }).ok();
                                                        }
                                                    }
                                                });
                                            });

                                            ui.add_space(8.0);

                                            // Full-width Brightness Slider without name/value text (0 turns OFF)
                                            if group.action.bri.is_some() {
                                                let mut bri = group.action.bri.unwrap_or(0);
                                                ui.spacing_mut().slider_width = ui.available_width();
                                                let slider_res = ui.add_sized(
                                                    egui::vec2(ui.available_width(), 16.0),
                                                    egui::Slider::new(&mut bri, 0..=254).show_value(false),
                                                );
                                                if slider_res.changed() {
                                                    let is_on = bri > 0;
                                                    group.action.bri = Some(bri);
                                                    group.action.on = Some(is_on);
                                                    self.group_last_local_edit.insert(item.id.clone(), Instant::now());

                                                    let mut action = serde_json::json!({ "bri": bri, "on": is_on });
                                                    if bri == 0 {
                                                        action = serde_json::json!({ "on": false });
                                                    }
                                                    self.bg_tx.send(BgMessage::SetGroupAction {
                                                        group_id: item.id.clone(),
                                                        action,
                                                    }).ok();
                                                }
                                            }
                                        } else {
                                            ui.horizontal(|ui| {
                                                ui.label("Unknown Group");
                                            });
                                        }
                                    }
                                });
                            });

                        // Mouse wheel adjustments over card
                        let card_rect = card_response.response.rect;

                        // Live reordering: while a widget is being dragged, swapping it with
                        // whichever card the pointer is currently over gives immediate visual
                        // feedback, rather than relying on a one-shot drag distance threshold.
                        if let Some(drag_idx) = self.dashboard_drag {
                            if drag_idx != idx
                                && ui.rect_contains_pointer(card_rect)
                                && ui.input(|i| i.pointer.button_down(egui::PointerButton::Primary))
                            {
                                swap_indices = Some((drag_idx, idx));
                                self.dashboard_drag = Some(idx);
                            }
                        }

                        if ui.rect_contains_pointer(card_rect) {
                            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
                            if scroll.abs() > 0.0 {
                                let raw_diff = scroll * 0.30; // scaled down to be fine-grained
                                let diff = if raw_diff.abs() > 0.0 {
                                    if raw_diff > 0.0 {
                                        raw_diff.max(1.0) as i16
                                    } else {
                                        raw_diff.min(-1.0) as i16
                                    }
                                } else {
                                    0
                                };
                                if item.r#type == "light" {
                                    if let Some(light) = self.lights.get_mut(&item.id) {
                                        let current_bri = light.state.bri.unwrap_or(0) as i16;
                                        let new_bri = (current_bri + diff).clamp(0, 254) as u8;
                                        light.state.bri = Some(new_bri);
                                        let mut state = serde_json::json!({ "bri": new_bri });
                                        if !light.state.on.unwrap_or(false) && scroll > 0.0 {
                                            light.state.on = Some(true);
                                            state["on"] = serde_json::json!(true);
                                        }
                                        if new_bri == 0 {
                                            light.state.on = Some(false);
                                            state = serde_json::json!({ "on": false });
                                        }
                                        self.bg_tx.send(BgMessage::SetLightState {
                                            light_id: item.id.clone(),
                                            state,
                                        }).ok();
                                    }
                                } else if item.r#type == "group" {
                                    if let Some(group) = self.groups.get_mut(&item.id) {
                                        let current_bri = group.action.bri.unwrap_or(0) as i16;
                                        let new_bri = (current_bri + diff).clamp(0, 254) as u8;
                                        group.action.bri = Some(new_bri);
                                        let mut state = serde_json::json!({ "bri": new_bri });
                                        if !group.action.on.unwrap_or(false) && scroll > 0.0 {
                                            group.action.on = Some(true);
                                            state["on"] = serde_json::json!(true);
                                        }
                                        if new_bri == 0 {
                                            group.action.on = Some(false);
                                            state = serde_json::json!({ "on": false });
                                        }
                                        self.bg_tx.send(BgMessage::SetGroupAction {
                                            group_id: item.id.clone(),
                                            action: state,
                                        }).ok();
                                    }
                                }
                            }

                            // Double Click to Toggle using full card rectangle checks (without blocking click/drag of subwidgets)
                            if ui.input(|i| i.pointer.button_double_clicked(egui::PointerButton::Primary)) {
                                if item.r#type == "light" {
                                    if let Some(light) = self.lights.get_mut(&item.id) {
                                        let next_state = !light.state.on.unwrap_or(false);
                                        light.state.on = Some(next_state);
                                        self.bg_tx.send(BgMessage::SetLightState {
                                            light_id: item.id.clone(),
                                            state: serde_json::json!({ "on": next_state }),
                                        }).ok();
                                    }
                                } else if item.r#type == "group" {
                                    if let Some(group) = self.groups.get_mut(&item.id) {
                                        let next_state = !group.action.on.unwrap_or(false);
                                        group.action.on = Some(next_state);
                                        self.bg_tx.send(BgMessage::SetGroupAction {
                                            group_id: item.id.clone(),
                                            action: serde_json::json!({ "on": next_state }),
                                        }).ok();
                                    }
                                }
                            }
                        }

                        // Wrap column layout
                        if (idx + 1) % columns == 0 {
                            ui.end_row();
                        }
                    }

                    if let Some((i1, i2)) = swap_indices {
                        self.config.dashboard_layout.swap(i1, i2);
                        self.config.save().ok();
                    }
                });
        });

        // Floating add button using Area to overlay on top of the ScrollArea
        egui::Area::new(egui::Id::new("floating_add_widgets_button"))
            .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-24.0, -24.0))
            .show(ui.ctx(), |ui| {
                ui.scope(|ui| {
                    let btn_size = egui::vec2(44.0, 44.0);
                    let (rect, response) = ui.allocate_exact_size(btn_size, egui::Sense::click());
                    if response.clicked() {
                        self.add_widgets.is_open = true;
                        self.add_widgets.selected_lights.clear();
                        self.add_widgets.selected_groups.clear();
                    }
                    
                    let bg_color = if response.is_pointer_button_down_on() {
                        egui::Color32::from_rgb(150, 70, 230)
                    } else if response.hovered() {
                        egui::Color32::from_rgb(180, 100, 255)
                    } else {
                        egui::Color32::from_rgb(168, 85, 247)
                    };
                    
                    ui.painter().circle_filled(rect.center(), 22.0, bg_color);
                    
                    let thickness = 2.5;
                    let line_len = 14.0;
                    let center = rect.center();
                    
                    ui.painter().line_segment(
                        [
                            egui::pos2(center.x - line_len / 2.0, center.y),
                            egui::pos2(center.x + line_len / 2.0, center.y),
                        ],
                        egui::Stroke::new(thickness, egui::Color32::WHITE),
                    );
                    
                    ui.painter().line_segment(
                        [
                            egui::pos2(center.x, center.y - line_len / 2.0),
                            egui::pos2(center.x, center.y + line_len / 2.0),
                        ],
                        egui::Stroke::new(thickness, egui::Color32::WHITE),
                    );
                });
            });
    }

    fn draw_midi_mapping_tab(&mut self, ui: &mut egui::Ui) {
        let spacing = 16.0;
        // Below this width the two columns' contents start clipping, so keep them at
        // their natural size and let a horizontal scrollbar appear instead of cutting them off.
        let min_total_width = 800.0;
        let total_width = ui.available_width().max(min_total_width);
        let col_width = (total_width - spacing) / 2.0;
        let available_height = ui.available_height() - 8.0;

        egui::ScrollArea::horizontal().auto_shrink([false, false]).show(ui, |ui| {
        ui.allocate_ui(egui::vec2(total_width, available_height), |ui| {
        // `horizontal_centered` (unlike plain `horizontal`) reserves the full available
        // height up front instead of growing to fit content, so the columns below can
        // actually stretch to match the window instead of being clamped to one line's height.
        ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing.x = spacing;

            // Column 1 Frame: MIDI Inputs
            let frame1 = egui::Frame::window(ui.style())
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(44, 48, 68)))
                .shadow(ui.style().visuals.window_shadow)
                .inner_margin(egui::Margin::symmetric(16, 16));

            ui.allocate_ui(egui::vec2(col_width, ui.available_height()), |ui| {
                frame1.show(ui, |ui| {
                    ui.set_min_height(ui.available_height());
                    let inner_w = col_width - 34.0;
                    ui.set_min_width(inner_w);
                    ui.set_max_width(inner_w);
                    ui.vertical(|ui| {
                        // Title bar with right-aligned status
                        ui.horizontal(|ui| {
                            ui.heading("MIDI Inputs");
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                let status_color = if self.midi_status.contains("Active") {
                                    egui::Color32::from_rgb(34, 197, 94) // Green
                                } else if self.midi_status.contains("Connecting") {
                                    egui::Color32::from_rgb(234, 179, 8) // Yellow
                                } else {
                                    egui::Color32::from_rgb(120, 120, 120) // Gray
                                };
                                ui.colored_label(status_color, &self.midi_status);
                                ui.label("Status:");
                            });
                        });
                        ui.add_space(10.0);

                        // Active Device Label and Selector
                        ui.label("Active Device");
                        ui.add_space(4.0);
                        
                        let mut current_port = self.config.selected_device.clone();
                        ui.scope(|ui| {
                            // Larger padding for dropdown button to seem more significant
                            ui.style_mut().spacing.button_padding = egui::vec2(12.0, 8.0);
                            
                            let combo = egui::ComboBox::from_id_salt("active_device_dropdown")
                                .width(ui.available_width() - 8.0) // Grow to full width
                                .selected_text(if current_port.is_empty() { "Select MIDI Device" } else { &current_port });
                                
                            combo.show_ui(ui, |ui| {
                                for port in &self.midi_ports {
                                    ui.selectable_value(&mut current_port, port.clone(), port);
                                }
                            });
                        });

                        if current_port != self.config.selected_device {
                            self.config.selected_device = current_port.clone();
                            self.config.save().ok();
                            self.bg_tx.send(BgMessage::UpdateConfig(self.config.clone())).ok();
                            self.bg_tx.send(BgMessage::ChangeMidiPort(current_port)).ok();
                            self.midi_status = "Connecting...".to_string();
                        }

                        ui.add_space(20.0);
                        ui.heading("Activity Log");
                        ui.add_space(5.0);

                        // Activity log canvas - fills the remaining height of the column
                        // instead of staying pinned to a fixed minimum.
                        let log_height = ui.available_height();
                        egui::Frame::dark_canvas(ui.style())
                            .fill(egui::Color32::from_rgb(8, 10, 18))
                            .show(ui, |ui| {
                                ui.set_min_size(egui::vec2(ui.available_width(), log_height.max(160.0)));
                                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                                    if self.midi_log.is_empty() {
                                        ui.centered_and_justified(|ui| {
                                            ui.weak("No MIDI messages received yet. Move a knob/slider or press a key.");
                                        });
                                    } else {
                                        for entry in &self.midi_log {
                                            ui.horizontal(|ui| {
                                                ui.colored_label(egui::Color32::from_rgb(120, 120, 140), format!("[{}]", entry.time));
                                                ui.colored_label(egui::Color32::from_rgb(168, 85, 247), &entry.event_key);
                                                ui.label(format!("val: {}", entry.value));
                                                
                                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                    if ui.small_button("Bind").clicked() {
                                                        self.mapping_creator.is_open = true;
                                                        self.mapping_creator.event_key = entry.event_key.clone();
                                                    }
                                                });
                                            });
                                        }
                                    }
                                });
                            });
                    });
                });
            });

            // Column 2 Frame: Active Mappings
            let frame2 = egui::Frame::window(ui.style())
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(44, 48, 68)))
                .shadow(ui.style().visuals.window_shadow)
                .inner_margin(egui::Margin::symmetric(16, 16));

            ui.allocate_ui(egui::vec2(col_width, ui.available_height()), |ui| {
                frame2.show(ui, |ui| {
                    ui.set_min_height(ui.available_height());
                    let inner_w = col_width - 34.0;
                    ui.set_min_width(inner_w);
                    ui.set_max_width(inner_w);
                    ui.vertical(|ui| {
                        ui.heading("Active Mappings");
                        ui.add_space(10.0);

                        let selected_device = self.config.selected_device.clone();
                        let device_mappings: HashMap<String, Mapping> = self.config.mappings
                            .get(&selected_device)
                            .cloned()
                            .unwrap_or_default();

                        if device_mappings.is_empty() {
                            ui.weak("No mappings configured for this controller. Press 'Bind' in the Activity Log to create one.");
                            return;
                        }

                        let mut edit_key = None;

                        let table_width = inner_w - 50.0; // Subtract padding/scrollbar
                        let col3_w = 56.0_f32.min(table_width * 0.15);
                        let remaining = table_width - col3_w;
                        let col0_w = remaining * 0.25;
                        let col1_w = remaining * 0.40;
                        let col2_w = remaining * 0.35;

                        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                            egui::Grid::new("active_mappings_table")
                                .num_columns(4)
                                .spacing(egui::vec2(12.0, 10.0))
                                .striped(true)
                                .show(ui, |ui| {
                                    // Table Headers
                                    ui.allocate_ui(egui::vec2(col0_w, 20.0), |ui| {
                                        ui.set_min_width(col0_w);
                                        ui.label(egui::RichText::new("MIDI Event").strong());
                                    });
                                    ui.allocate_ui(egui::vec2(col1_w, 20.0), |ui| {
                                        ui.set_min_width(col1_w);
                                        ui.label(egui::RichText::new("Target").strong());
                                    });
                                    ui.allocate_ui(egui::vec2(col2_w, 20.0), |ui| {
                                        ui.set_min_width(col2_w);
                                        ui.label(egui::RichText::new("Action").strong());
                                    });
                                    ui.allocate_ui(egui::vec2(col3_w, 20.0), |ui| {
                                        ui.set_min_width(col3_w);
                                        ui.label(egui::RichText::new("Actions").strong());
                                    });
                                    ui.end_row();

                                    for (event, mapping) in device_mappings.iter() {
                                        // Column 1: Event
                                        ui.allocate_ui(egui::vec2(col0_w, 28.0), |ui| {
                                            ui.set_min_width(col0_w);
                                            ui.colored_label(egui::Color32::from_rgb(168, 85, 247), event);
                                        });

                                        // Column 2: Target
                                        ui.allocate_ui(egui::vec2(col1_w, 28.0), |ui| {
                                            ui.set_min_width(col1_w);
                                            ui.horizontal(|ui| {
                                                let icon = match mapping.target_type.as_str() {
                                                    "light" => "💡",
                                                    "group" => "📦",
                                                    _ => "🎬",
                                                };
                                                ui.label(icon);

                                                let target_name = match mapping.target_type.as_str() {
                                                    "light" => self.lights.get(&mapping.target_id).map(|l| l.name.clone()).unwrap_or_else(|| format!("Light {}", mapping.target_id)),
                                                    "group" => self.groups.get(&mapping.target_id).map(|g| g.name.clone()).unwrap_or_else(|| format!("Group {}", mapping.target_id)),
                                                    _ => self.scenes.get(&mapping.target_id).map(|s| s.name.clone()).unwrap_or_else(|| format!("Scene {}", mapping.target_id)),
                                                };
                                                ui.label(&target_name);
                                            });
                                        });

                                        // Column 3: Action
                                        ui.allocate_ui(egui::vec2(col2_w, 28.0), |ui| {
                                            ui.set_min_width(col2_w);
                                            ui.label(&mapping.action);
                                        });

                                        // Column 4: Edit & Delete
                                        ui.allocate_ui(egui::vec2(col3_w, 28.0), |ui| {
                                            ui.set_min_width(col3_w);
                                            ui.spacing_mut().item_spacing.x = 4.0;
                                            ui.horizontal(|ui| {
                                                if ui.add_sized(egui::vec2(24.0, 24.0), egui::Button::new("✏").frame(false)).clicked() {
                                                    edit_key = Some(event.clone());
                                                }
                                                if ui.add_sized(egui::vec2(24.0, 24.0), egui::Button::new("🗑").frame(false)).clicked() {
                                                    self.request_confirm(
                                                        "Delete MIDI Bind?",
                                                        &format!("Remove the binding for \"{}\"? This cannot be undone.", event),
                                                        ConfirmAction::RemoveMapping(event.clone()),
                                                    );
                                                }
                                            });
                                        });

                                        ui.end_row();
                                    }
                                });
                        });

                        if let Some(key) = edit_key {
                            if let Some(maps) = self.config.mappings.get(&selected_device) {
                                if let Some(map) = maps.get(&key) {
                                    self.mapping_creator.is_open = true;
                                    self.mapping_creator.event_key = key.clone();
                                    self.mapping_creator.target_type = map.target_type.clone();
                                    self.mapping_creator.target_id = map.target_id.clone();
                                    self.mapping_creator.action = map.action.clone();
                                    self.mapping_creator.invert = map.invert;
                                    self.mapping_creator.auto_on = map.auto_on;
                                }
                            }
                        }
                    });
                });
            });
        });
        });
        });
    }

    fn draw_settings_modal(&mut self, ctx: &egui::Context) {
        if !self.settings.is_open {
            return;
        }

        let mut open = true;
        egui::Window::new("Settings")
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .default_width(420.0)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                let full_width = ui.available_width();

                egui::Grid::new("settings_grid")
                    .spacing(egui::vec2(10.0, 15.0))
                    .show(ui, |ui| {
                        ui.label("Hue Bridge IP:");
                        ui.label(&self.config.bridge_ip);
                        ui.end_row();
                    });

                ui.add_space(15.0);

                ui.label("Configuration Path:");
                ui.add_space(2.0);
                if let Some(path) = AppConfig::get_config_path() {
                    ui.weak(path.to_string_lossy().to_string());
                } else {
                    ui.weak("In-Memory Only");
                }

                ui.add_space(20.0);

                let mut autostart = self.config.autostart;
                if ui.checkbox(&mut autostart, "Launch on Startup").changed() {
                    self.config.autostart = autostart;
                    self.config.save().ok();
                    self.bg_tx.send(BgMessage::UpdateConfig(self.config.clone())).ok();

                    // Integrate auto-launch crate trigger here
                    if let Some(path) = std::env::current_exe().ok() {
                        let app_path = path.to_string_lossy();
                        let auto = auto_launch::AutoLaunchBuilder::new()
                            .set_app_name("HueMIDIty")
                            .set_app_path(&app_path)
                            .set_macos_launch_mode(auto_launch::MacOSLaunchMode::LaunchAgent)
                            .build();
                        if let Ok(auto) = auto {
                            if autostart {
                                auto.enable().ok();
                            } else {
                                auto.disable().ok();
                            }
                        }
                    }
                }

                ui.add_space(10.0);

                let mut forget_clicked = false;
                ui.scope(|ui| {
                    let danger_color = egui::Color32::from_rgb(0xff, 0x33, 0x66);
                    let danger_fill = egui::Color32::from_rgb(0x1d, 0x1b, 0x23);
                    let danger_fill_hovered = egui::Color32::from_rgb(0x22, 0x20, 0x27);
                    ui.style_mut().visuals.widgets.inactive.bg_fill = danger_fill;
                    ui.style_mut().visuals.widgets.inactive.weak_bg_fill = danger_fill;
                    ui.style_mut().visuals.widgets.hovered.bg_fill = danger_fill_hovered;
                    ui.style_mut().visuals.widgets.hovered.weak_bg_fill = danger_fill_hovered;
                    ui.style_mut().visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, danger_color);
                    ui.style_mut().visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, danger_color);
                    ui.style_mut().visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, danger_color);
                    ui.style_mut().visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, danger_color);
                    ui.style_mut().visuals.widgets.hovered.expansion = 0.0;
                    ui.style_mut().spacing.button_padding = egui::vec2(12.0, 8.0);
                    if ui.button("🗑 Forget Hue Bridge").clicked() {
                        forget_clicked = true;
                    }
                });
                if forget_clicked {
                    self.request_confirm(
                        "Forget Hue Bridge?",
                        "This will disconnect from the Hue Bridge and remove its saved IP address and credentials. You will need to re-pair with the bridge to control your lights again.",
                        ConfirmAction::ForgetBridge,
                    );
                }

                ui.add_space(6.0);

                ui.allocate_ui_with_layout(egui::vec2(full_width, 28.0), egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.style_mut().spacing.button_padding = egui::vec2(12.0, 6.0);
                    ui.style_mut().spacing.item_spacing.y = 0.0;

                    if ui.button("Close").clicked() {
                        self.settings.is_open = false;
                    }

                    if ui.button("Quit App").clicked() {
                        ui.ctx().send_viewport_cmd_to(egui::ViewportId::ROOT, egui::ViewportCommand::Close);
                    }
                });
            });

        if !open {
            self.settings.is_open = false;
        }
    }

    fn draw_mapping_modal(&mut self, ctx: &egui::Context) {
        if !self.mapping_creator.is_open {
            return;
        }

        let mut open = true;
        egui::Window::new("Create MIDI Bind")
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .default_width(380.0)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                let full_width = ui.available_width();
                ui.style_mut().spacing.button_padding = egui::vec2(12.0, 10.0);

                ui.label(format!("Mapping Event: {}", self.mapping_creator.event_key));
                ui.add_space(10.0);

                // Target Type
                ui.label("Target Type");
                ui.style_mut().spacing.combo_height = 200.0;
                egui::ComboBox::from_id_salt("target_type_combo")
                    .width(full_width)
                    .selected_text(&self.mapping_creator.target_type)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.mapping_creator.target_type, "light".to_string(), "💡 Light");
                        ui.selectable_value(&mut self.mapping_creator.target_type, "group".to_string(), "📦 Group");
                        ui.selectable_value(&mut self.mapping_creator.target_type, "scene".to_string(), "🎬 Scene");
                    });

                ui.add_space(10.0);

                // Target Device List based on type
                let target_type = self.mapping_creator.target_type.clone();
                ui.label("Select Target");
                egui::ComboBox::from_id_salt("select_target_combo")
                    .width(full_width)
                    .selected_text(if self.mapping_creator.target_id.is_empty() {
                        "Choose Device/Scene..."
                    } else {
                        let id = &self.mapping_creator.target_id;
                        match target_type.as_str() {
                            "light" => self.lights.get(id).map(|l| l.name.as_str()).unwrap_or("Unknown Light"),
                            "group" => self.groups.get(id).map(|g| g.name.as_str()).unwrap_or("Unknown Group"),
                            _ => self.scenes.get(id).map(|s| s.name.as_str()).unwrap_or("Unknown Scene"),
                        }
                    })
                    .show_ui(ui, |ui| {
                        match target_type.as_str() {
                            "light" => {
                                let mut sorted: Vec<(&String, &Light)> = self.lights.iter().collect();
                                sorted.sort_by(|a, b| a.1.name.to_lowercase().cmp(&b.1.name.to_lowercase()));
                                for (id, light) in sorted {
                                    ui.selectable_value(&mut self.mapping_creator.target_id, id.clone(), &light.name);
                                }
                            }
                            "group" => {
                                let mut sorted: Vec<(&String, &Group)> = self.groups.iter().collect();
                                sorted.sort_by(|a, b| a.1.name.to_lowercase().cmp(&b.1.name.to_lowercase()));
                                for (id, group) in sorted {
                                    ui.selectable_value(&mut self.mapping_creator.target_id, id.clone(), &group.name);
                                }
                            }
                            _ => {
                                let mut sorted: Vec<(&String, &Scene)> = self.scenes.iter().collect();
                                sorted.sort_by(|a, b| a.1.name.to_lowercase().cmp(&b.1.name.to_lowercase()));
                                for (id, scene) in sorted {
                                    ui.selectable_value(&mut self.mapping_creator.target_id, id.clone(), &scene.name);
                                }
                            }
                        }
                    });

                ui.add_space(10.0);

                // Filter actions depending on Target capabilities
                let id = &self.mapping_creator.target_id;
                let capabilities = if target_type == "light" {
                    self.lights.get(id).map(|l| l.capabilities()).unwrap_or_default()
                } else if target_type == "group" {
                    // Group union capabilities (default to all if not resolved)
                    if let Some(group) = self.groups.get(id) {
                        let mut caps = HashSet::new();
                        for light_id in &group.lights {
                            if let Some(l) = self.lights.get(light_id) {
                                caps.extend(l.capabilities());
                            }
                        }
                        if caps.is_empty() {
                            let mut full = HashSet::new();
                            full.insert(Capability::Dim);
                            full.insert(Capability::Ct);
                            full.insert(Capability::Color);
                            full
                        } else {
                            caps
                        }
                    } else {
                        HashSet::new()
                    }
                } else {
                    HashSet::new()
                };

                // Action Selector
                ui.label("Action / Property");
                egui::ComboBox::from_id_salt("action_property_combo")
                    .width(full_width)
                    .selected_text(&self.mapping_creator.action)
                    .show_ui(ui, |ui| {
                        if target_type == "scene" {
                            self.mapping_creator.action = "Recall Scene".to_string();
                            ui.selectable_value(&mut self.mapping_creator.action, "Recall Scene".to_string(), "Recall Scene");
                        } else {
                            ui.selectable_value(&mut self.mapping_creator.action, "On/Off (Latch)".to_string(), "Toggle On/Off (Latch)");
                            ui.selectable_value(&mut self.mapping_creator.action, "On/Off (Momentary)".to_string(), "Toggle On/Off (Momentary)");

                            if capabilities.contains(&Capability::Dim) {
                                ui.selectable_value(&mut self.mapping_creator.action, "Brightness".to_string(), "Brightness");
                            }
                            if capabilities.contains(&Capability::Ct) {
                                ui.selectable_value(&mut self.mapping_creator.action, "Color Temperature".to_string(), "Color Temp");
                            }
                            if capabilities.contains(&Capability::Color) {
                                ui.selectable_value(&mut self.mapping_creator.action, "Hue".to_string(), "Hue");
                                ui.selectable_value(&mut self.mapping_creator.action, "Saturation".to_string(), "Saturation");
                                ui.selectable_value(&mut self.mapping_creator.action, "Red Component".to_string(), "Red Component");
                                ui.selectable_value(&mut self.mapping_creator.action, "Green Component".to_string(), "Green Component");
                                ui.selectable_value(&mut self.mapping_creator.action, "Blue Component".to_string(), "Blue Component");
                            }
                        }
                    });

                ui.add_space(8.0);

                // Toggle options
                if target_type != "scene" && !self.mapping_creator.action.starts_with("On/Off") {
                    ui.checkbox(&mut self.mapping_creator.invert, "Invert Control (127 -> Min, 0 -> Max)");
                    ui.checkbox(&mut self.mapping_creator.auto_on, "Auto-On (turn device ON when adjusting)");
                }

                ui.add_space(6.0);

                ui.allocate_ui_with_layout(egui::vec2(full_width, 28.0), egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.style_mut().spacing.button_padding = egui::vec2(12.0, 6.0);
                    ui.style_mut().spacing.item_spacing.y = 0.0;

                    if ui.button("Save Mapping").clicked() {
                        if !self.mapping_creator.target_id.is_empty() {
                            let active_device = self.config.selected_device.clone();
                            let device_maps = self.config.mappings.entry(active_device).or_insert_with(HashMap::new);
                            device_maps.insert(self.mapping_creator.event_key.clone(), Mapping {
                                target_type: self.mapping_creator.target_type.clone(),
                                target_id: self.mapping_creator.target_id.clone(),
                                action: self.mapping_creator.action.clone(),
                                invert: self.mapping_creator.invert,
                                auto_on: self.mapping_creator.auto_on,
                            });
                            self.config.save().ok();
                            self.bg_tx.send(BgMessage::UpdateConfig(self.config.clone())).ok();
                            self.mapping_creator.is_open = false;
                        }
                    }

                    if ui.button("Cancel").clicked() {
                        self.mapping_creator.is_open = false;
                    }
                });
            });

        if !open {
            self.mapping_creator.is_open = false;
        }
    }

    fn draw_widget_modal(&mut self, ctx: &egui::Context) {
        if !self.add_widgets.is_open {
            return;
        }

        let mut open = true;
        egui::Window::new("Add Widgets to Dashboard")
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .default_width(450.0)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.add_widgets.search_query)
                        .hint_text("Filter list by name...")
                );
                ui.add_space(10.0);

                // Create checklist of devices not in layout
                let current_layout_ids: HashSet<String> = self.config.dashboard_layout.iter().map(|item| item.id.clone()).collect();
                let query = self.add_widgets.search_query.to_lowercase();

                let mut sorted_lights: Vec<(&String, &Light)> = self.lights.iter().collect();
                sorted_lights.sort_by(|a, b| a.1.name.to_lowercase().cmp(&b.1.name.to_lowercase()));

                let mut sorted_groups: Vec<(&String, &Group)> = self.groups.iter().collect();
                sorted_groups.sort_by(|a, b| a.1.name.to_lowercase().cmp(&b.1.name.to_lowercase()));

                egui::ScrollArea::vertical()
                    .max_height(250.0)
                    .min_scrolled_width(420.0)
                    .show(ui, |ui| {
                        ui.heading("Lights");
                        for (id, light) in sorted_lights {
                            if !light.name.to_lowercase().contains(&query) {
                                continue;
                            }
                            let added = current_layout_ids.contains(id);
                            let row_w = ui.available_width();
                            let (rect, response) = ui.allocate_exact_size(egui::vec2(row_w, 32.0), egui::Sense::click());
                            
                            if added {
                                let visuals = ui.style().visuals.widgets.noninteractive;
                                let mut child_ui = ui.child_ui(rect, egui::Layout::left_to_right(egui::Align::Center), None);
                                child_ui.add_space(8.0);
                                let mut val = true;
                                child_ui.add_enabled(false, egui::Checkbox::new(&mut val, ""));
                                child_ui.colored_label(visuals.text_color(), format!("{} (added)", light.name));
                            } else {
                                let mut selected = self.add_widgets.selected_lights.contains(id);
                                
                                if response.clicked() {
                                    selected = !selected;
                                    if selected {
                                        self.add_widgets.selected_lights.insert(id.clone());
                                    } else {
                                        self.add_widgets.selected_lights.remove(id);
                                    }
                                }
                                
                                let bg_fill = if response.hovered() {
                                    egui::Color32::from_rgba_unmultiplied(168, 85, 247, 25)
                                } else if selected {
                                    egui::Color32::from_rgba_unmultiplied(168, 85, 247, 15)
                                } else {
                                    egui::Color32::TRANSPARENT
                                };
                                if bg_fill != egui::Color32::TRANSPARENT {
                                    ui.painter().rect_filled(rect, 4.0, bg_fill);
                                }
                                
                                let mut child_ui = ui.child_ui(rect, egui::Layout::left_to_right(egui::Align::Center), None);
                                child_ui.add_space(8.0);
                                let mut cb_val = selected;
                                if child_ui.checkbox(&mut cb_val, "").changed() {
                                    selected = cb_val;
                                    if selected {
                                        self.add_widgets.selected_lights.insert(id.clone());
                                    } else {
                                        self.add_widgets.selected_lights.remove(id);
                                    }
                                }
                                child_ui.label(&light.name);
                            }
                        }

                        ui.add_space(10.0);
                        ui.heading("Groups");
                        for (id, group) in sorted_groups {
                            if !group.name.to_lowercase().contains(&query) {
                                continue;
                            }
                            let added = current_layout_ids.contains(id);
                            let row_w = ui.available_width();
                            let (rect, response) = ui.allocate_exact_size(egui::vec2(row_w, 32.0), egui::Sense::click());
                            
                            if added {
                                let visuals = ui.style().visuals.widgets.noninteractive;
                                let mut child_ui = ui.child_ui(rect, egui::Layout::left_to_right(egui::Align::Center), None);
                                child_ui.add_space(8.0);
                                let mut val = true;
                                child_ui.add_enabled(false, egui::Checkbox::new(&mut val, ""));
                                child_ui.colored_label(visuals.text_color(), format!("{} (added)", group.name));
                            } else {
                                let mut selected = self.add_widgets.selected_groups.contains(id);
                                
                                if response.clicked() {
                                    selected = !selected;
                                    if selected {
                                        self.add_widgets.selected_groups.insert(id.clone());
                                    } else {
                                        self.add_widgets.selected_groups.remove(id);
                                    }
                                }
                                
                                let bg_fill = if response.hovered() {
                                    egui::Color32::from_rgba_unmultiplied(168, 85, 247, 25)
                                } else if selected {
                                    egui::Color32::from_rgba_unmultiplied(168, 85, 247, 15)
                                } else {
                                    egui::Color32::TRANSPARENT
                                };
                                if bg_fill != egui::Color32::TRANSPARENT {
                                    ui.painter().rect_filled(rect, 4.0, bg_fill);
                                }
                                
                                let mut child_ui = ui.child_ui(rect, egui::Layout::left_to_right(egui::Align::Center), None);
                                child_ui.add_space(8.0);
                                let mut cb_val = selected;
                                if child_ui.checkbox(&mut cb_val, "").changed() {
                                    selected = cb_val;
                                    if selected {
                                        self.add_widgets.selected_groups.insert(id.clone());
                                    } else {
                                        self.add_widgets.selected_groups.remove(id);
                                    }
                                }
                                child_ui.label(&group.name);
                            }
                        }
                    });

                ui.add_space(6.0);

                let bottom_row_width = ui.available_width();
                ui.allocate_ui_with_layout(egui::vec2(bottom_row_width, 28.0), egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.style_mut().spacing.button_padding = egui::vec2(12.0, 6.0);
                    ui.style_mut().spacing.item_spacing.y = 0.0;

                    if ui.button("Add Selected").clicked() {
                        for id in &self.add_widgets.selected_lights {
                            self.config.dashboard_layout.push(DashboardItem {
                                r#type: "light".to_string(),
                                id: id.clone(),
                            });
                        }
                        for id in &self.add_widgets.selected_groups {
                            self.config.dashboard_layout.push(DashboardItem {
                                r#type: "group".to_string(),
                                id: id.clone(),
                            });
                        }
                        self.config.save().ok();
                        self.bg_tx.send(BgMessage::UpdateConfig(self.config.clone())).ok();
                        self.add_widgets.is_open = false;
                    }

                    if ui.button("Cancel").clicked() {
                        self.add_widgets.is_open = false;
                    }
                });
            });

        if !open {
            self.add_widgets.is_open = false;
        }
    }
}

/// Stable id for the one real application window, which is created/destroyed on demand
/// as a deferred child viewport of the always-present ghost root (see [`RootApp`]).
pub fn main_viewport_id() -> egui::ViewportId {
    egui::ViewportId::from_hash_of("huemidity_main_window")
}

impl HueMIDItyApp {
    /// Draws the actual application UI. Called every frame the main window is visible,
    /// from inside the deferred child viewport's callback (see [`RootApp::update`]) - so
    /// `ctx.input(..)`/`ctx.send_viewport_cmd(..)` here refer to that child window, not
    /// the ghost root.
    fn draw_child_window(&mut self, ctx: &egui::Context) {
        ctx.style_mut(|style| {
            style.visuals.interact_cursor = Some(egui::CursorIcon::PointingHand);
            style.interaction.selectable_labels = false;
        });

        self.check_channels(ctx);

        // Clicking the window's own close (X) button hides it to the tray instead of
        // exiting (RootApp simply stops re-creating this child viewport, which tears
        // down the native window cleanly - no hidden/minimized window state involved
        // at all). If the tray failed to initialize, there'd be no way to bring the
        // window back, so fall back to a real close in that case.
        if ctx.input(|i| i.viewport().close_requested()) {
            if self.tray_available {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.window_visible = false;
            } else {
                ctx.send_viewport_cmd_to(egui::ViewportId::ROOT, egui::ViewportCommand::Close);
            }
        }

        // Check connection state to decide whether to draw onboarding overlay
        match &self.connection_state {
            BridgeConnectionState::Connected { .. } => {
                self.draw_main_app(ctx);
            }
            _ => {
                self.draw_onboarding(ctx);
            }
        }
    }
}

/// The top-level `eframe::App`. Its own viewport (the root) is a permanently invisible,
/// taskbar-less "ghost" window that exists only to keep the event loop alive - on
/// Windows, a window that's actually hidden (`Visible(false)`/minimized) stops getting
/// `WM_PAINT`, which would starve `update()` of any chance to ever show it again. The
/// real UI lives in a deferred child viewport that this `update()` creates or omits each
/// frame depending on `window_visible`; omitting it tears the native window down
/// completely (no taskbar entry), and calling it again creates a fresh one from scratch.
pub struct RootApp {
    pub app: Arc<Mutex<HueMIDItyApp>>,
    pub icon: Option<Arc<egui::IconData>>,
    last_visible: bool,
}

impl RootApp {
    pub fn new(app: Arc<Mutex<HueMIDItyApp>>, icon: Option<Arc<egui::IconData>>) -> Self {
        Self {
            app,
            icon,
            last_visible: true,
        }
    }
}

impl eframe::App for RootApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let window_visible = {
            let mut app = self.app.lock().unwrap();

            // System tray menu events handler. Lives here (root) rather than in the
            // child window's draw code, since the root keeps ticking unconditionally -
            // the child window may not exist at all when these events arrive.
            while let Ok(event) = muda::MenuEvent::receiver().try_recv() {
                let id_str = &event.id.0;
                if id_str == &app.tray_show_hide_id {
                    app.window_visible = !app.window_visible;
                } else if id_str == &app.tray_quit_id {
                    ctx.send_viewport_cmd_to(egui::ViewportId::ROOT, egui::ViewportCommand::Close);
                }
            }

            // A single left-click on the tray icon toggles the window. React on the
            // button-up transition only - mouse-down and mouse-up each produce their
            // own Click event, so reacting to both would toggle twice (net no-op).
            while let Ok(event) = tray_icon::TrayIconEvent::receiver().try_recv() {
                if let tray_icon::TrayIconEvent::Click {
                    button: tray_icon::MouseButton::Left,
                    button_state: tray_icon::MouseButtonState::Up,
                    ..
                } = event
                {
                    app.window_visible = !app.window_visible;
                }
            }

            app.window_visible
        };

        if window_visible {
            let app_handle = Arc::clone(&self.app);
            let viewport_id = main_viewport_id();
            let mut builder = egui::ViewportBuilder::default()
                .with_title("HueMIDIty")
                .with_inner_size([720.0, 480.0])
                .with_min_inner_size([640.0, 400.0])
                .with_active(true)
                .with_taskbar(true);
            if let Some(icon) = &self.icon {
                builder = builder.with_icon(Arc::clone(icon));
            }

            ctx.show_viewport_deferred(viewport_id, builder, move |ctx, _class| {
                app_handle.lock().unwrap().draw_child_window(ctx);
            });

            // Just transitioned from hidden to visible (tray click, menu, or startup) -
            // bring the freshly (re)created window to the front. This is a brand new
            // native window in direct response to user input, so Windows allows it to
            // take foreground focus normally.
            if !self.last_visible {
                ctx.send_viewport_cmd_to(viewport_id, egui::ViewportCommand::Focus);
            }
        }
        self.last_visible = window_visible;
    }
}
