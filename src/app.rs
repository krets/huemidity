use eframe::egui;
use crate::config::{AppConfig, DashboardItem, Mapping};
use crate::hue::BridgeConnectionState;
use crate::hue::types::{Light, Group, Scene, Capability};
use crate::midi::{MidiEvent, get_midi_ports};

pub fn setup_custom_theme(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    
    // Background and frame colors
    visuals.panel_fill = egui::Color32::from_rgb(15, 17, 26); // #0F111A deep dark
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


use std::collections::{HashMap, HashSet};
use std::time::Instant;
use tokio::sync::mpsc::{UnboundedSender as Sender, UnboundedReceiver as Receiver};

#[derive(Clone, Debug)]
pub enum GuiMessage {
    MidiActivity(MidiEvent),
    HueConnectionState(BridgeConnectionState),
    DevicesRefreshed {
        lights: HashMap<String, Light>,
        groups: HashMap<String, Group>,
        scenes: HashMap<String, Scene>,
    },
    Error(String),
}

#[derive(Clone, Debug)]
pub enum BgMessage {
    ConnectManual(String),
    StartAutoDiscovery,
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
    RecallScene {
        group_id: String,
        scene_id: String,
    },
    ChangeMidiPort(String),
    MidiInputReceived(MidiEvent),
}

#[derive(Clone, PartialEq, Eq)]
pub enum Tab {
    Dashboard,
    MidiMapping,
    Settings,
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

    // Tray Menu IDs
    pub tray_show_hide_id: String,
    pub tray_quit_id: String,

    // Timing and UI effects
    pub last_port_refresh: Instant,
    pub manual_ip_input: String,
}

impl HueMIDItyApp {
    pub fn new(
        config: AppConfig,
        gui_rx: Receiver<GuiMessage>,
        bg_tx: Sender<BgMessage>,
        tray_show_hide_id: String,
        tray_quit_id: String,
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
                countdown: 30,
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
            tray_show_hide_id,
            tray_quit_id,
            last_port_refresh: Instant::now(),
        }
    }

    fn check_channels(&mut self) {
        while let Ok(msg) = self.gui_rx.try_recv() {
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
                GuiMessage::HueConnectionState(state) => {
                    self.connection_state = state.clone();
                    match &state {
                        BridgeConnectionState::Connected { ip, username } => {
                            self.config.bridge_ip = ip.clone();
                            self.config.bridge_username = username.clone();
                            self.config.save().ok();
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
                GuiMessage::DevicesRefreshed { lights, groups, scenes } => {
                    self.lights = lights;
                    self.groups = groups;
                    self.scenes = scenes;
                }
                GuiMessage::Error(err_msg) => {
                    self.midi_status = format!("Conflict: {}", err_msg);
                }
            }
        }
    }

    pub fn draw_onboarding(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(80.0);
                
                // Pulsing title / icon
                ui.heading(
                    egui::RichText::new("HueMIDIty")
                        .size(36.0)
                        .color(egui::Color32::from_rgb(168, 85, 247))
                        .strong()
                );
                ui.label("Bind physical MIDI inputs to Philips Hue lighting controls.");
                ui.add_space(40.0);

                match &self.connection_state {
                    BridgeConnectionState::Searching => {
                        ui.add(egui::Spinner::new().size(32.0));
                        ui.add_space(20.0);
                        ui.label("Searching for Philips Hue Bridge on your local network...");
                        ui.add_space(30.0);

                        if ui.button("Enter IP Manually").clicked() {
                            self.connection_state = BridgeConnectionState::Idle;
                        }
                    }
                    BridgeConnectionState::Idle => {
                        ui.label("Enter the IP address of your Philips Hue Bridge:");
                        ui.add_space(10.0);

                        ui.horizontal(|ui| {
                            ui.shrink_width_to_current();
                            ui.add_space(ui.available_width() / 4.0);
                            ui.text_edit_singleline(&mut self.manual_ip_input);
                        });

                        ui.add_space(20.0);
                        ui.horizontal(|ui| {
                            ui.shrink_width_to_current();
                            ui.add_space(ui.available_width() / 3.0);
                            if ui.button("Connect").clicked() {
                                if !self.manual_ip_input.is_empty() {
                                    self.bg_tx.send(BgMessage::ConnectManual(self.manual_ip_input.trim().to_string())).ok();
                                }
                            }
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
                        );
                        ui.add_space(20.0);
                        
                        // ProgressBar countdown
                        let progress = *countdown as f32 / 30.0;
                        ui.add(egui::ProgressBar::new(progress).text(format!("{} seconds remaining", countdown)));
                        ui.add_space(15.0);
                        ui.label(format!("Bridge IP: {}", ip));

                        ui.add_space(20.0);
                        if ui.button("Back / Change IP").clicked() {
                            self.connection_state = BridgeConnectionState::Idle;
                        }
                    }
                    BridgeConnectionState::Error(err) => {
                        ui.colored_label(egui::Color32::from_rgb(239, 68, 68), "Connection Error");
                        ui.label(format!("Unable to connect to the Hue Bridge: {}", err));
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
    }

    pub fn draw_main_app(&mut self, ctx: &egui::Context) {
        // Refresh midi ports every 5 seconds
        if self.last_port_refresh.elapsed().as_secs() > 5 {
            self.midi_ports = get_midi_ports();
            self.last_port_refresh = Instant::now();
        }

        // Top panel header
        egui::TopBottomPanel::top("top_header").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("HueMIDIty");
                ui.add_space(20.0);

                // Tab buttons
                ui.selectable_value(&mut self.active_tab, Tab::Dashboard, "📋 Dashboard");
                ui.selectable_value(&mut self.active_tab, Tab::MidiMapping, "🎹 MIDI Mapping");
                ui.selectable_value(&mut self.active_tab, Tab::Settings, "⚙ Settings");

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

        // Modals
        self.draw_mapping_modal(ctx);
        self.draw_widget_modal(ctx);

        // Content
        egui::CentralPanel::default().show(ctx, |ui| {
            match self.active_tab {
                Tab::Dashboard => self.draw_dashboard_tab(ui),
                Tab::MidiMapping => self.draw_midi_mapping_tab(ui),
                Tab::Settings => self.draw_settings_tab(ui),
            }
        });
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

        // Scrollable widgets grid
        egui::ScrollArea::vertical().show(ui, |ui| {
            let width = ui.available_width();
            let columns = (width / 240.0).floor().max(1.0) as usize;
            
            // Simple grid layout
            egui::Grid::new("dashboard_grid")
                .spacing(egui::vec2(12.0, 12.0))
                .show(ui, |ui| {
                    let mut remove_index = None;
                    let mut swap_indices = None;
                    
                    let layout_items = self.config.dashboard_layout.clone();
                    for (idx, item) in layout_items.iter().enumerate() {
                        let card_response = egui::Frame::window(ui.style())
                            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(44, 48, 68)))
                            .show(ui, |ui| {
                                ui.set_min_size(egui::vec2(220.0, 110.0));
                                ui.vertical(|ui| {
                                    // Header of card
                                    ui.horizontal(|ui| {
                                        let drag_handle = ui.label("⠿");
                                        let drag_response = ui.interact(
                                            drag_handle.rect,
                                            ui.id().with("drag").with(idx),
                                            egui::Sense::drag(),
                                        );

                                        if drag_response.dragged() {
                                            // Handle drag reordering
                                            let delta = drag_response.drag_delta();
                                            if delta.y > 20.0 && idx < layout_items.len() - 1 {
                                                swap_indices = Some((idx, idx + 1));
                                            } else if delta.y < -20.0 && idx > 0 {
                                                swap_indices = Some((idx, idx - 1));
                                            }
                                        }

                                        let name = if item.r#type == "light" {
                                            self.lights.get(&item.id).map(|l| l.name.clone()).unwrap_or_else(|| format!("Light {}", item.id))
                                        } else {
                                            self.groups.get(&item.id).map(|g| g.name.clone()).unwrap_or_else(|| format!("Group {}", item.id))
                                        };

                                        ui.heading(egui::RichText::new(&name).size(14.0).strong());
                                        
                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                            if ui.button("×").clicked() {
                                                remove_index = Some(idx);
                                            }
                                        });
                                    });

                                    ui.add_space(8.0);

                                    // Display controls depending on light or group
                                    if item.r#type == "light" {
                                        if let Some(light) = self.lights.get(&item.id) {
                                            let mut is_on = light.state.on.unwrap_or(false);
                                            if ui.checkbox(&mut is_on, "Power").changed() {
                                                self.bg_tx.send(BgMessage::SetLightState {
                                                    light_id: item.id.clone(),
                                                    state: serde_json::json!({ "on": is_on }),
                                                }).ok();
                                            }

                                            // Brightness Slider
                                            if light.state.bri.is_some() {
                                                let mut bri = light.state.bri.unwrap_or(0);
                                                if ui.add(egui::Slider::new(&mut bri, 0..=254).text("Brightness")).changed() {
                                                    self.bg_tx.send(BgMessage::SetLightState {
                                                        light_id: item.id.clone(),
                                                        state: serde_json::json!({ "bri": bri }),
                                                    }).ok();
                                                }
                                            }
                                        } else {
                                            ui.colored_label(egui::Color32::from_rgb(120, 120, 120), "Offline");
                                        }
                                    } else if item.r#type == "group" {
                                        if let Some(group) = self.groups.get(&item.id) {
                                            let mut is_on = group.action.on.unwrap_or(false);
                                            if ui.checkbox(&mut is_on, "Power").changed() {
                                                self.bg_tx.send(BgMessage::SetGroupAction {
                                                    group_id: item.id.clone(),
                                                    action: serde_json::json!({ "on": is_on }),
                                                }).ok();
                                            }

                                            // Brightness
                                            if group.action.bri.is_some() {
                                                let mut bri = group.action.bri.unwrap_or(0);
                                                if ui.add(egui::Slider::new(&mut bri, 0..=254).text("Brightness")).changed() {
                                                    self.bg_tx.send(BgMessage::SetGroupAction {
                                                        group_id: item.id.clone(),
                                                        action: serde_json::json!({ "bri": bri }),
                                                    }).ok();
                                                }
                                            }
                                        } else {
                                            ui.label("Unknown Group");
                                        }
                                    }
                                });
                            });

                        // Mouse wheel adjustments over card
                        if card_response.response.hovered() {
                            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
                            if scroll.abs() > 0.0 {
                                let diff = if scroll > 0.0 { 15i16 } else { -15i16 };
                                if item.r#type == "light" {
                                    if let Some(light) = self.lights.get(&item.id) {
                                        let current_bri = light.state.bri.unwrap_or(0) as i16;
                                        let new_bri = (current_bri + diff).clamp(0, 254) as u8;
                                        let mut state = serde_json::json!({ "bri": new_bri });
                                        if !light.state.on.unwrap_or(false) && scroll > 0.0 {
                                            state["on"] = serde_json::json!(true);
                                        }
                                        self.bg_tx.send(BgMessage::SetLightState {
                                            light_id: item.id.clone(),
                                            state,
                                        }).ok();
                                    }
                                } else if item.r#type == "group" {
                                    if let Some(group) = self.groups.get(&item.id) {
                                        let current_bri = group.action.bri.unwrap_or(0) as i16;
                                        let new_bri = (current_bri + diff).clamp(0, 254) as u8;
                                        let mut state = serde_json::json!({ "bri": new_bri });
                                        if !group.action.on.unwrap_or(false) && scroll > 0.0 {
                                            state["on"] = serde_json::json!(true);
                                        }
                                        self.bg_tx.send(BgMessage::SetGroupAction {
                                            group_id: item.id.clone(),
                                            action: state,
                                        }).ok();
                                    }
                                }
                            }

                            // Double Click to Toggle
                            if card_response.response.double_clicked() {
                                if item.r#type == "light" {
                                    if let Some(light) = self.lights.get(&item.id) {
                                        let next_state = !light.state.on.unwrap_or(false);
                                        self.bg_tx.send(BgMessage::SetLightState {
                                            light_id: item.id.clone(),
                                            state: serde_json::json!({ "on": next_state }),
                                        }).ok();
                                    }
                                } else if item.r#type == "group" {
                                    if let Some(group) = self.groups.get(&item.id) {
                                        let next_state = !group.action.on.unwrap_or(false);
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

                    if let Some(idx) = remove_index {
                        self.config.dashboard_layout.remove(idx);
                        self.config.save().ok();
                    }
                    if let Some((i1, i2)) = swap_indices {
                        self.config.dashboard_layout.swap(i1, i2);
                        self.config.save().ok();
                    }
                });
        });

        // Floating add button
        ui.with_layout(egui::Layout::bottom_up(egui::Align::Max), |ui| {
            ui.add_space(20.0);
            if ui.button(egui::RichText::new(" ➕ Add Widgets ").strong().size(16.0)).clicked() {
                self.add_widgets.is_open = true;
                self.add_widgets.selected_lights.clear();
                self.add_widgets.selected_groups.clear();
            }
        });
    }

    fn draw_midi_mapping_tab(&mut self, ui: &mut egui::Ui) {
        ui.columns(2, |columns| {
            // Column 1: Live inputs & Log
            columns[0].vertical(|ui| {
                ui.heading("MIDI Inputs");
                ui.add_space(10.0);

                // Port Selector Dropdown
                let mut current_port = self.config.selected_device.clone();
                egui::ComboBox::from_label("Active Device")
                    .selected_text(if current_port.is_empty() { "Select MIDI Device" } else { &current_port })
                    .show_ui(ui, |ui| {
                        for port in &self.midi_ports {
                            ui.selectable_value(&mut current_port, port.clone(), port);
                        }
                    });

                if current_port != self.config.selected_device {
                    self.config.selected_device = current_port.clone();
                    self.config.save().ok();
                    self.bg_tx.send(BgMessage::ChangeMidiPort(current_port)).ok();
                    self.midi_status = "Connecting...".to_string();
                }

                ui.add_space(10.0);
                
                // Status badge
                let status_color = if self.midi_status.contains("Active") {
                    egui::Color32::from_rgb(34, 197, 94) // Green
                } else if self.midi_status.contains("Connecting") {
                    egui::Color32::from_rgb(234, 179, 8) // Yellow
                } else {
                    egui::Color32::from_rgb(120, 120, 120) // Gray
                };
                ui.horizontal(|ui| {
                    ui.label("Status:");
                    ui.colored_label(status_color, &self.midi_status);
                });

                ui.add_space(20.0);
                ui.heading("Activity Log");
                ui.add_space(5.0);

                // Monospace scrollable log box
                egui::Frame::dark_canvas(ui.style())
                    .fill(egui::Color32::from_rgb(8, 10, 18))
                    .show(ui, |ui| {
                        ui.set_min_size(egui::vec2(ui.available_width(), 160.0));
                        egui::ScrollArea::vertical().show(ui, |ui| {
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

            // Column 2: Mappings list
            columns[1].vertical(|ui| {
                ui.heading("Active Mappings");
                ui.add_space(10.0);

                let selected_device = self.config.selected_device.clone();
                let device_mappings = self.config.mappings.entry(selected_device.clone()).or_insert_with(HashMap::new);

                if device_mappings.is_empty() {
                    ui.weak("No mappings configured for this controller. Press 'Bind' in the Activity Log to create one.");
                    return;
                }

                let mut delete_key = None;
                let mut edit_key = None;

                egui::ScrollArea::vertical().show(ui, |ui| {
                    for (event, mapping) in device_mappings.iter() {
                        egui::Frame::window(ui.style())
                            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(44, 48, 68)))
                            .show(ui, |ui| {
                                ui.set_min_size(egui::vec2(ui.available_width(), 48.0));
                                ui.horizontal(|ui| {
                                    ui.colored_label(egui::Color32::from_rgb(168, 85, 247), event);
                                    
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
                                    ui.separator();
                                    ui.label(format!("Action: {}", mapping.action));

                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        if ui.small_button("🗑").clicked() {
                                            delete_key = Some(event.clone());
                                        }
                                        if ui.small_button("✏").clicked() {
                                            edit_key = Some(event.clone());
                                        }
                                    });
                                });
                            });
                        ui.add_space(4.0);
                    }
                });

                if let Some(key) = delete_key {
                    if let Some(maps) = self.config.mappings.get_mut(&selected_device) {
                        maps.remove(&key);
                        self.config.save().ok();
                    }
                }

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
    }

    fn draw_settings_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading("Settings");
        ui.add_space(20.0);

        egui::Grid::new("settings_grid")
            .spacing(egui::vec2(10.0, 15.0))
            .show(ui, |ui| {
                ui.label("Hue Bridge IP:");
                ui.label(&self.config.bridge_ip);
                ui.end_row();

                ui.label("Registered Username:");
                ui.label(if self.config.bridge_username.is_empty() { "None" } else { &self.config.bridge_username });
                ui.end_row();

                ui.label("Configuration Path:");
                if let Some(path) = AppConfig::get_config_path() {
                    ui.weak(path.to_string_lossy().to_string());
                } else {
                    ui.weak("In-Memory Only");
                }
                ui.end_row();
            });

        ui.add_space(30.0);
        
        let mut autostart = self.config.autostart;
        if ui.checkbox(&mut autostart, "Launch on Startup").changed() {
            self.config.autostart = autostart;
            self.config.save().ok();
            
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

        ui.add_space(40.0);

        if ui.button(" Forget Hue Bridge").clicked() {
            self.bg_tx.send(BgMessage::ForgetBridge).ok();
            self.config.bridge_ip.clear();
            self.config.bridge_username.clear();
            self.config.save().ok();
            self.connection_state = BridgeConnectionState::Idle;
        }

        ui.add_space(10.0);
        if ui.button("Quit App").clicked() {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn draw_mapping_modal(&mut self, ctx: &egui::Context) {
        if !self.mapping_creator.is_open {
            return;
        }

        egui::Window::new("Create MIDI Bind")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label(format!("Mapping Event: {}", self.mapping_creator.event_key));
                ui.add_space(10.0);

                // Target Type
                egui::ComboBox::from_label("Target Type")
                    .selected_text(&self.mapping_creator.target_type)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.mapping_creator.target_type, "light".to_string(), "💡 Light");
                        ui.selectable_value(&mut self.mapping_creator.target_type, "group".to_string(), "📦 Group");
                        ui.selectable_value(&mut self.mapping_creator.target_type, "scene".to_string(), "🎬 Scene");
                    });

                ui.add_space(8.0);

                // Target Device List based on type
                let target_type = self.mapping_creator.target_type.clone();
                egui::ComboBox::from_label("Select Target")
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
                                for (id, light) in &self.lights {
                                    ui.selectable_value(&mut self.mapping_creator.target_id, id.clone(), &light.name);
                                }
                            }
                            "group" => {
                                for (id, group) in &self.groups {
                                    ui.selectable_value(&mut self.mapping_creator.target_id, id.clone(), &group.name);
                                }
                            }
                            _ => {
                                for (id, scene) in &self.scenes {
                                    ui.selectable_value(&mut self.mapping_creator.target_id, id.clone(), &scene.name);
                                }
                            }
                        }
                    });

                ui.add_space(8.0);

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
                egui::ComboBox::from_label("Action / Property")
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

                ui.add_space(15.0);

                ui.horizontal(|ui| {
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
                            self.mapping_creator.is_open = false;
                        }
                    }

                    if ui.button("Cancel").clicked() {
                        self.mapping_creator.is_open = false;
                    }
                });
            });
    }

    fn draw_widget_modal(&mut self, ctx: &egui::Context) {
        if !self.add_widgets.is_open {
            return;
        }

        egui::Window::new("Add Widgets to Dashboard")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                ui.text_edit_singleline(&mut self.add_widgets.search_query);
                ui.add_space(10.0);

                // Create checklist of devices not in layout
                let current_layout_ids: HashSet<String> = self.config.dashboard_layout.iter().map(|item| item.id.clone()).collect();
                let query = self.add_widgets.search_query.to_lowercase();

                egui::ScrollArea::vertical().max_height(250.0).show(ui, |ui| {
                    ui.heading("Lights");
                    for (id, light) in &self.lights {
                        if !light.name.to_lowercase().contains(&query) {
                            continue;
                        }
                        let added = current_layout_ids.contains(id);
                        if added {
                            ui.add_enabled(false, egui::Checkbox::new(&mut true, format!("{} (added)", light.name)));
                        } else {
                            let mut selected = self.add_widgets.selected_lights.contains(id);
                            if ui.checkbox(&mut selected, &light.name).changed() {
                                if selected {
                                    self.add_widgets.selected_lights.insert(id.clone());
                                } else {
                                    self.add_widgets.selected_lights.remove(id);
                                }
                            }
                        }
                    }

                    ui.add_space(10.0);
                    ui.heading("Groups");
                    for (id, group) in &self.groups {
                        if !group.name.to_lowercase().contains(&query) {
                            continue;
                        }
                        let added = current_layout_ids.contains(id);
                        if added {
                            ui.add_enabled(false, egui::Checkbox::new(&mut true, format!("{} (added)", group.name)));
                        } else {
                            let mut selected = self.add_widgets.selected_groups.contains(id);
                            if ui.checkbox(&mut selected, &group.name).changed() {
                                if selected {
                                    self.add_widgets.selected_groups.insert(id.clone());
                                } else {
                                    self.add_widgets.selected_groups.remove(id);
                                }
                            }
                        }
                    }
                });

                ui.add_space(15.0);

                ui.horizontal(|ui| {
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
                        self.add_widgets.is_open = false;
                    }

                    if ui.button("Cancel").clicked() {
                        self.add_widgets.is_open = false;
                    }
                });
            });
    }
}

impl eframe::App for HueMIDItyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.check_channels();

        // System tray menu events handler
        while let Ok(event) = muda::MenuEvent::receiver().try_recv() {
            let id_str = &event.id.0;
            if id_str == &self.tray_show_hide_id {
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            } else if id_str == &self.tray_quit_id {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
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
