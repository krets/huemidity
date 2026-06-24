#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // Hide console window on Windows in release

mod config;
mod hue;
mod midi;
mod tray;
mod app;

use eframe::egui;
use config::AppConfig;
use hue::{HueClient, BridgeConnectionState};
use midi::MidiListener;
use app::{GuiMessage, BgMessage, HueMIDItyApp};

use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender as Sender, UnboundedReceiver as Receiver};

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

fn merge_json_objects(a: &mut serde_json::Value, b: serde_json::Value) {
    if let (Some(a_obj), Some(b_obj)) = (a.as_object_mut(), b.as_object()) {
        for (k, v) in b_obj {
            a_obj.insert(k.clone(), v.clone());
        }
    } else {
        *a = b;
    }
}

// Background manager task
#[allow(unused_variables, unused_assignments)]
async fn run_bg_worker(
    mut config: AppConfig,
    mut bg_rx: Receiver<BgMessage>,
    gui_tx: Sender<GuiMessage>,
    bg_tx_self: Sender<BgMessage>,
    ctx: egui::Context,
) {
    let hue_client = HueClient::new();
    let mut active_listener: Option<MidiListener> = None;

    // Cache of devices to perform toggles and HSB mappings
    let mut lights_cache = HashMap::new();
    let mut groups_cache = HashMap::new();
    let mut scenes_cache = HashMap::new();

    // Throttling maps
    let mut pending_lights: HashMap<String, serde_json::Value> = HashMap::new();
    let mut pending_groups: HashMap<String, serde_json::Value> = HashMap::new();
    let mut last_light_sent: HashMap<String, std::time::Instant> = HashMap::new();
    let mut last_group_sent: HashMap<String, std::time::Instant> = HashMap::new();

    // Check if we need to search or poll
    let mut connection_state = if !config.bridge_ip.is_empty() && !config.bridge_username.is_empty() {
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
        BridgeConnectionState::Searching
    };

    // Intervals
    let mut pairing_interval = tokio::time::interval(Duration::from_secs(1));
    let mut refresh_interval = tokio::time::interval(Duration::from_secs(10));
    let mut throttle_interval = tokio::time::interval(Duration::from_millis(20));

    loop {
        tokio::select! {
            // Periodic tasks
            _ = pairing_interval.tick() => {
                if let BridgeConnectionState::NeedsLink { ip, mut countdown } = connection_state.clone() {
                    if countdown > 0 {
                        countdown -= 1;
                        if countdown % 3 == 0 {
                            match hue_client.register_app(&ip).await {
                                Ok(Some(username)) => {
                                    connection_state = BridgeConnectionState::Connected { ip: ip.clone(), username: username.clone() };
                                    config.bridge_ip = ip;
                                    config.bridge_username = username;
                                    config.save().ok();
                                    gui_tx.send(GuiMessage::HueConnectionState(connection_state.clone())).ok();
                                    ctx.request_repaint();
                                }
                                Ok(None) => {
                                    connection_state = BridgeConnectionState::NeedsLink { ip, countdown };
                                    gui_tx.send(GuiMessage::HueConnectionState(connection_state.clone())).ok();
                                    ctx.request_repaint();
                                }
                                Err(e) => {
                                    connection_state = BridgeConnectionState::Error(e.to_string());
                                    gui_tx.send(GuiMessage::HueConnectionState(connection_state.clone())).ok();
                                    ctx.request_repaint();
                                }
                            }
                        } else {
                            connection_state = BridgeConnectionState::NeedsLink { ip, countdown };
                            gui_tx.send(GuiMessage::HueConnectionState(connection_state.clone())).ok();
                            ctx.request_repaint();
                        }
                    } else {
                        connection_state = BridgeConnectionState::Idle;
                        gui_tx.send(GuiMessage::HueConnectionState(connection_state.clone())).ok();
                        ctx.request_repaint();
                    }
                }
            }

            _ = refresh_interval.tick() => {
                if let BridgeConnectionState::Connected { ip, username } = &connection_state {
                    if let Ok(l) = hue_client.fetch_lights(ip, username).await {
                        lights_cache = l.clone();
                    }
                    if let Ok(g) = hue_client.fetch_groups(ip, username).await {
                        groups_cache = g.clone();
                    }
                    if let Ok(s) = hue_client.fetch_scenes(ip, username).await {
                        scenes_cache = s.clone();
                    }
                    // Keep GUI in sync
                    gui_tx.send(GuiMessage::DevicesRefreshed {
                        lights: lights_cache.clone(),
                        groups: groups_cache.clone(),
                        scenes: scenes_cache.clone(),
                    }).ok();
                    ctx.request_repaint();
                }
            }

            _ = throttle_interval.tick() => {
                if let BridgeConnectionState::Connected { ip, username } = &connection_state {
                    let now = std::time::Instant::now();
                    
                    // 1. Process pending lights
                    let mut lights_to_send = Vec::new();
                    for (light_id, state) in &pending_lights {
                        let last_sent = last_light_sent.get(light_id);
                        let can_send = match last_sent {
                            Some(&instant) => now.duration_since(instant) >= Duration::from_millis(100),
                            None => true,
                        };
                        if can_send {
                            lights_to_send.push((light_id.clone(), state.clone()));
                        }
                    }
                    for (light_id, state) in lights_to_send {
                        pending_lights.remove(&light_id);
                        last_light_sent.insert(light_id.clone(), now);
                        
                        let client = hue_client.clone();
                        let ip_clone = ip.clone();
                        let username_clone = username.clone();
                        tokio::spawn(async move {
                            client.set_light_state(&ip_clone, &username_clone, &light_id, &state).await.ok();
                        });
                    }

                    // 2. Process pending groups
                    let mut groups_to_send = Vec::new();
                    for (group_id, action) in &pending_groups {
                        let last_sent = last_group_sent.get(group_id);
                        let can_send = match last_sent {
                            Some(&instant) => now.duration_since(instant) >= Duration::from_millis(500),
                            None => true,
                        };
                        if can_send {
                            groups_to_send.push((group_id.clone(), action.clone()));
                        }
                    }
                    for (group_id, action) in groups_to_send {
                        pending_groups.remove(&group_id);
                        last_group_sent.insert(group_id.clone(), now);
                        
                        let client = hue_client.clone();
                        let ip_clone = ip.clone();
                        let username_clone = username.clone();
                        tokio::spawn(async move {
                            client.set_group_action(&ip_clone, &username_clone, &group_id, &action).await.ok();
                        });
                    }
                }
            }

            // GUI command receiver
            msg_opt = bg_rx.recv() => {
                let msg = match msg_opt {
                    Some(m) => m,
                    None => break, // Channel closed
                };

                match msg {
                    BgMessage::StartAutoDiscovery => {
                        connection_state = BridgeConnectionState::Searching;
                        gui_tx.send(GuiMessage::HueConnectionState(connection_state.clone())).ok();
                        ctx.request_repaint();

                        match hue_client.discover_bridges().await {
                            Ok(ips) => {
                                if let Some(ip) = ips.first() {
                                    connection_state = BridgeConnectionState::NeedsLink { ip: ip.clone(), countdown: 30 };
                                    config.bridge_ip = ip.clone();
                                    config.save().ok();
                                } else {
                                    connection_state = BridgeConnectionState::Idle;
                                }
                            }
                            Err(_) => {
                                connection_state = BridgeConnectionState::Idle;
                            }
                        }
                        gui_tx.send(GuiMessage::HueConnectionState(connection_state.clone())).ok();
                        ctx.request_repaint();
                    }

                    BgMessage::ConnectManual(ip) => {
                        connection_state = BridgeConnectionState::NeedsLink { ip: ip.clone(), countdown: 30 };
                        config.bridge_ip = ip;
                        config.save().ok();
                        gui_tx.send(GuiMessage::HueConnectionState(connection_state.clone())).ok();
                        ctx.request_repaint();
                    }

                    BgMessage::ForgetBridge => {
                        connection_state = BridgeConnectionState::Idle;
                        config.bridge_ip.clear();
                        config.bridge_username.clear();
                        config.save().ok();
                        pending_lights.clear();
                        pending_groups.clear();
                        last_light_sent.clear();
                        last_group_sent.clear();
                        gui_tx.send(GuiMessage::HueConnectionState(connection_state.clone())).ok();
                        ctx.request_repaint();
                    }

                    BgMessage::RefreshDevices => {
                        if let BridgeConnectionState::Connected { ip, username } = &connection_state {
                            let lights = hue_client.fetch_lights(ip, username).await.unwrap_or_default();
                            let groups = hue_client.fetch_groups(ip, username).await.unwrap_or_default();
                            let scenes = hue_client.fetch_scenes(ip, username).await.unwrap_or_default();

                            lights_cache = lights.clone();
                            groups_cache = groups.clone();
                            scenes_cache = scenes.clone();

                            gui_tx.send(GuiMessage::DevicesRefreshed { lights, groups, scenes }).ok();
                            ctx.request_repaint();
                        }
                    }

                    BgMessage::SetLightState { light_id, state } => {
                        // Update local cache
                        if let Some(light) = lights_cache.get_mut(&light_id) {
                            if let Some(on) = state.get("on").and_then(|o| o.as_bool()) {
                                light.state.on = Some(on);
                            }
                            if let Some(bri) = state.get("bri").and_then(|b| b.as_u64()) {
                                light.state.bri = Some(bri as u8);
                            }
                            if let Some(hue) = state.get("hue").and_then(|h| h.as_u64()) {
                                light.state.hue = Some(hue as u16);
                            }
                            if let Some(sat) = state.get("sat").and_then(|s| s.as_u64()) {
                                light.state.sat = Some(sat as u8);
                            }
                            if let Some(ct) = state.get("ct").and_then(|c| c.as_u64()) {
                                light.state.ct = Some(ct as u16);
                            }
                        }
                        // Queue for rate-limited dispatch
                        if let Some(existing) = pending_lights.get_mut(&light_id) {
                            merge_json_objects(existing, state);
                        } else {
                            pending_lights.insert(light_id.clone(), state);
                        }
                    }

                    BgMessage::SetGroupAction { group_id, action } => {
                        // Update local cache
                        if let Some(group) = groups_cache.get_mut(&group_id) {
                            if let Some(on) = action.get("on").and_then(|o| o.as_bool()) {
                                group.action.on = Some(on);
                            }
                            if let Some(bri) = action.get("bri").and_then(|b| b.as_u64()) {
                                group.action.bri = Some(bri as u8);
                            }
                            if let Some(hue) = action.get("hue").and_then(|h| h.as_u64()) {
                                group.action.hue = Some(hue as u16);
                            }
                            if let Some(sat) = action.get("sat").and_then(|s| s.as_u64()) {
                                group.action.sat = Some(sat as u8);
                            }
                            if let Some(ct) = action.get("ct").and_then(|c| c.as_u64()) {
                                group.action.ct = Some(ct as u16);
                            }
                        }
                        // Queue for rate-limited dispatch
                        if let Some(existing) = pending_groups.get_mut(&group_id) {
                            merge_json_objects(existing, action);
                        } else {
                            pending_groups.insert(group_id.clone(), action);
                        }
                    }

                    BgMessage::RecallScene { group_id, scene_id } => {
                        if let BridgeConnectionState::Connected { ip, username } = &connection_state {
                            let body = serde_json::json!({ "scene": scene_id });
                            let client = hue_client.clone();
                            let ip_clone = ip.clone();
                            let username_clone = username.clone();
                            tokio::spawn(async move {
                                client.set_group_action(&ip_clone, &username_clone, &group_id, &body).await.ok();
                            });
                        }
                    }

                    BgMessage::ChangeMidiPort(port_name) => {
                        active_listener = None; // Drop old connection
                        let bg_tx_clone = bg_tx_self.clone();
                        match MidiListener::new(&port_name, move |event| {
                            bg_tx_clone.send(BgMessage::MidiInputReceived(event)).ok();
                        }) {
                            Ok(listener) => {
                                active_listener = Some(listener);
                                config.selected_device = port_name.clone();
                                config.save().ok();
                                gui_tx.send(GuiMessage::MidiStatus("Live Input: Active".to_string())).ok();
                            }
                            Err(e) => {
                                gui_tx.send(GuiMessage::MidiStatus(format!("Conflict: Device Busy"))).ok();
                                gui_tx.send(GuiMessage::Error(e.to_string())).ok();
                            }
                        }
                        ctx.request_repaint();
                    }

                    BgMessage::UpdateConfig(new_config) => {
                        config = new_config;
                    }

                    BgMessage::MidiInputReceived(event) => {
                        // Forward to GUI
                        gui_tx.send(GuiMessage::MidiActivity(event.clone())).ok();
                        ctx.request_repaint();

                        // Execute mapping
                        if let Some(device_mappings) = config.mappings.get(&event.device_name) {
                            if let Some(mapping) = device_mappings.get(&event.event_key) {
                                if let BridgeConnectionState::Connected { ip, username } = &connection_state {
                                    
                                    // Scale values
                                    let raw_val = event.value;
                                    let mut state_body = serde_json::json!({});
                                    let is_cc = event.event_key.starts_with("CC");

                                    match mapping.action.as_str() {
                                        "Brightness" => {
                                            let mut bri = (raw_val as f32 * (254.0 / 127.0)) as u8;
                                            if mapping.invert {
                                                bri = 254 - bri;
                                            }
                                            if bri == 0 {
                                                state_body["on"] = serde_json::json!(false);
                                            } else {
                                                state_body["bri"] = serde_json::json!(bri);
                                                if mapping.auto_on {
                                                    state_body["on"] = serde_json::json!(true);
                                                }
                                            }
                                        }
                                        "Hue" => {
                                            let mut hue = (raw_val as f32 * (65535.0 / 127.0)) as u16;
                                            if mapping.invert {
                                                hue = 65535 - hue;
                                            }
                                            state_body["hue"] = serde_json::json!(hue);
                                            if mapping.auto_on {
                                                state_body["on"] = serde_json::json!(true);
                                            }
                                        }
                                        "Saturation" => {
                                            let mut sat = (raw_val as f32 * (254.0 / 127.0)) as u8;
                                            if mapping.invert {
                                                sat = 254 - sat;
                                            }
                                            state_body["sat"] = serde_json::json!(sat);
                                            if mapping.auto_on {
                                                state_body["on"] = serde_json::json!(true);
                                            }
                                        }
                                        "Color Temperature" => {
                                            let mut ct = (153.0 + (raw_val as f32 * (347.0 / 127.0))) as u16;
                                            if mapping.invert {
                                                ct = 500 - (ct - 153);
                                            }
                                            state_body["ct"] = serde_json::json!(ct);
                                            if mapping.auto_on {
                                                state_body["on"] = serde_json::json!(true);
                                            }
                                        }
                                        "On/Off (Latch)" => {
                                            let turn_on = if is_cc {
                                                raw_val >= 64
                                            } else {
                                                // Note On with velocity > 0 toggles
                                                if raw_val > 0 {
                                                    let currently_on = if mapping.target_type == "light" {
                                                        lights_cache.get(&mapping.target_id).and_then(|l| l.state.on).unwrap_or(false)
                                                    } else {
                                                        groups_cache.get(&mapping.target_id).and_then(|g| g.action.on).unwrap_or(false)
                                                    };
                                                    !currently_on
                                                } else {
                                                    continue; // Release event, ignore
                                                }
                                            };
                                            state_body["on"] = serde_json::json!(turn_on);
                                        }
                                        "On/Off (Momentary)" => {
                                            let active = if is_cc { raw_val >= 64 } else { raw_val > 0 };
                                            if active {
                                                let currently_on = if mapping.target_type == "light" {
                                                    lights_cache.get(&mapping.target_id).and_then(|l| l.state.on).unwrap_or(false)
                                                } else {
                                                    groups_cache.get(&mapping.target_id).and_then(|g| g.action.on).unwrap_or(false)
                                                };
                                                state_body["on"] = serde_json::json!(!currently_on);
                                            } else {
                                                continue; // Release event, ignore
                                            }
                                        }
                                        "Recall Scene" => {
                                            let active = if is_cc { raw_val >= 64 } else { raw_val > 0 };
                                            if active {
                                                let body = serde_json::json!({ "scene": mapping.target_id });
                                                // Scenes require group ID, fallback to Group 0 (all lights) if scene target doesn't specify a group
                                                let group_id = "0".to_string();
                                                let client = hue_client.clone();
                                                let ip_clone = ip.clone();
                                                let username_clone = username.clone();
                                                tokio::spawn(async move {
                                                    client.set_group_action(&ip_clone, &username_clone, &group_id, &body).await.ok();
                                                });
                                            }
                                            continue;
                                        }
                                        "Red Component" | "Green Component" | "Blue Component" => {
                                            // Scale to 0.0 - 1.0
                                            let mut component_val = raw_val as f32 / 127.0;
                                            if mapping.invert {
                                                component_val = 1.0 - component_val;
                                            }

                                            // Get current HSB from cache
                                            let (h_val, s_val, v_val) = if mapping.target_type == "light" {
                                                if let Some(light) = lights_cache.get(&mapping.target_id) {
                                                    let h = light.state.hue.unwrap_or(0) as f32 / 65535.0 * 360.0;
                                                    let s = light.state.sat.unwrap_or(0) as f32 / 254.0;
                                                    let v = light.state.bri.unwrap_or(0) as f32 / 254.0;
                                                    (h, s, v)
                                                } else { (0.0, 0.0, 1.0) }
                                            } else {
                                                if let Some(group) = groups_cache.get(&mapping.target_id) {
                                                    let h = group.action.hue.unwrap_or(0) as f32 / 65535.0 * 360.0;
                                                    let s = group.action.sat.unwrap_or(0) as f32 / 254.0;
                                                    let v = group.action.bri.unwrap_or(0) as f32 / 254.0;
                                                    (h, s, v)
                                                } else { (0.0, 0.0, 1.0) }
                                            };

                                            let (mut r, mut g, mut b) = hsv_to_rgb(h_val, s_val, v_val);
                                            match mapping.action.as_str() {
                                                "Red Component" => r = component_val,
                                                "Green Component" => g = component_val,
                                                _ => b = component_val,
                                            }

                                            let (new_h, new_s, new_v) = rgb_to_hsv(r, g, b);
                                            state_body["hue"] = serde_json::json!((new_h / 360.0 * 65535.0) as u16);
                                            state_body["sat"] = serde_json::json!((new_s * 254.0) as u8);
                                            state_body["bri"] = serde_json::json!((new_v * 254.0) as u8);
                                            if mapping.auto_on {
                                                state_body["on"] = serde_json::json!(true);
                                            }
                                        }
                                        _ => {}
                                    }

                                    // Dispatch Hue command
                                    if !state_body.as_object().unwrap().is_empty() {
                                        if mapping.target_type == "light" {
                                            // 1. Update cache immediately
                                            if let Some(light) = lights_cache.get_mut(&mapping.target_id) {
                                                if let Some(on) = state_body.get("on").and_then(|o| o.as_bool()) {
                                                    light.state.on = Some(on);
                                                }
                                                if let Some(bri) = state_body.get("bri").and_then(|b| b.as_u64()) {
                                                    light.state.bri = Some(bri as u8);
                                                }
                                                if let Some(hue) = state_body.get("hue").and_then(|h| h.as_u64()) {
                                                    light.state.hue = Some(hue as u16);
                                                }
                                                if let Some(sat) = state_body.get("sat").and_then(|s| s.as_u64()) {
                                                    light.state.sat = Some(sat as u8);
                                                }
                                                if let Some(ct) = state_body.get("ct").and_then(|c| c.as_u64()) {
                                                    light.state.ct = Some(ct as u16);
                                                }
                                            }
                                            // 2. Queue for sending
                                            if let Some(existing) = pending_lights.get_mut(&mapping.target_id) {
                                                merge_json_objects(existing, state_body);
                                            } else {
                                                pending_lights.insert(mapping.target_id.clone(), state_body);
                                            }
                                            // 3. Broadcast to GUI immediately
                                            gui_tx.send(GuiMessage::DevicesRefreshed {
                                                lights: lights_cache.clone(),
                                                groups: groups_cache.clone(),
                                                scenes: scenes_cache.clone(),
                                            }).ok();
                                            ctx.request_repaint();
                                        } else if mapping.target_type == "group" {
                                            // 1. Update cache immediately
                                            if let Some(group) = groups_cache.get_mut(&mapping.target_id) {
                                                if let Some(on) = state_body.get("on").and_then(|o| o.as_bool()) {
                                                    group.action.on = Some(on);
                                                }
                                                if let Some(bri) = state_body.get("bri").and_then(|b| b.as_u64()) {
                                                    group.action.bri = Some(bri as u8);
                                                }
                                                if let Some(hue) = state_body.get("hue").and_then(|h| h.as_u64()) {
                                                    group.action.hue = Some(hue as u16);
                                                }
                                                if let Some(sat) = state_body.get("sat").and_then(|s| s.as_u64()) {
                                                    group.action.sat = Some(sat as u8);
                                                }
                                                if let Some(ct) = state_body.get("ct").and_then(|c| c.as_u64()) {
                                                    group.action.ct = Some(ct as u16);
                                                }
                                            }
                                            // 2. Queue for sending
                                            if let Some(existing) = pending_groups.get_mut(&mapping.target_id) {
                                                merge_json_objects(existing, state_body);
                                            } else {
                                                pending_groups.insert(mapping.target_id.clone(), state_body);
                                            }
                                            // 3. Broadcast to GUI immediately
                                            gui_tx.send(GuiMessage::DevicesRefreshed {
                                                lights: lights_cache.clone(),
                                                groups: groups_cache.clone(),
                                                scenes: scenes_cache.clone(),
                                            }).ok();
                                            ctx.request_repaint();
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), eframe::Error> {
    // 1. Load config
    let config = AppConfig::load();

    // 2. Setup System Tray
    let tray = tray::setup_tray().ok();

    // 3. Create communication channels
    let (gui_tx, gui_rx) = unbounded_channel();
    let (bg_tx, bg_rx) = unbounded_channel();

    // 4. Setup eframe options
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("HueMIDIty")
            .with_inner_size([720.0, 480.0])
            .with_min_inner_size([640.0, 400.0])
            .with_active(true)
            .with_visible(true),
        ..Default::default()
    };

    // 5. Initialize tray variables
    let (show_hide_id, quit_id) = if let Some(ref t) = tray {
        (t.show_hide_id.clone(), t.quit_id.clone())
    } else {
        (String::new(), String::new())
    };

    let bg_tx_clone = bg_tx.clone();
    let gui_tx_clone = gui_tx.clone();

    // To pass the Context and start the background loop, we can defer spawning or pass a channel.
    // Let's spawn the background worker after we get the Context from eframe app setup!
    // Let's structure the eframe launch:
    eframe::run_native(
        "HueMIDIty",
        options,
        Box::new(move |cc| {
            // Apply custom styling
            app::setup_custom_theme(&cc.egui_ctx);

            // Spawn background task now that we have the Context
            let ctx_clone = cc.egui_ctx.clone();
            let conf_clone = config.clone();
            let bg_tx_self = bg_tx.clone();
            tokio::spawn(async move {
                run_bg_worker(conf_clone, bg_rx, gui_tx_clone, bg_tx_self, ctx_clone).await;
            });

            // Create App instance
            let app = HueMIDItyApp::new(
                config,
                gui_rx,
                bg_tx_clone,
                show_hide_id,
                quit_id,
            );

            // Keep reference to tray icon alive so it doesn't get dropped
            if let Some(t) = tray {
                // We can leak it or store it in window/app state.
                // Leaking is perfectly fine for application lifetime!
                Box::leak(Box::new(t));
            }

            Ok(Box::new(app) as Box<dyn eframe::App>)
        }),
    )
}
