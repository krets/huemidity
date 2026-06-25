use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
pub struct LightState {
    pub on: Option<bool>,
    pub bri: Option<u8>,
    pub hue: Option<u16>,
    pub sat: Option<u8>,
    pub ct: Option<u16>,
    pub xy: Option<Vec<f32>>,
    pub reachable: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Light {
    pub name: String,
    pub r#type: String,
    pub state: LightState,
    pub modelid: String,
    pub uniqueid: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
pub struct GroupAction {
    pub on: Option<bool>,
    pub bri: Option<u8>,
    pub hue: Option<u16>,
    pub sat: Option<u8>,
    pub ct: Option<u16>,
    pub xy: Option<Vec<f32>>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Group {
    pub name: String,
    pub r#type: String,
    pub lights: Vec<String>,
    pub action: GroupAction,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Scene {
    pub name: String,
    pub lights: Option<Vec<String>>,
    pub group: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum Capability {
    Dim,
    Ct,
    Color,
}

impl Light {
    pub fn capabilities(&self) -> HashSet<Capability> {
        let mut caps = HashSet::new();
        let state = &self.state;
        
        if state.bri.is_some() {
            caps.insert(Capability::Dim);
        }
        if state.ct.is_some() {
            caps.insert(Capability::Ct);
        }
        if state.hue.is_some() || state.sat.is_some() || state.xy.is_some() {
            caps.insert(Capability::Color);
        }
        
        // Fallback checks
        if caps.is_empty() {
            let t = self.r#type.to_lowercase();
            if t.contains("color") {
                caps.insert(Capability::Dim);
                caps.insert(Capability::Ct);
                caps.insert(Capability::Color);
            } else if t.contains("temp") {
                caps.insert(Capability::Dim);
                caps.insert(Capability::Ct);
            } else {
                caps.insert(Capability::Dim); // Default fallback
            }
        }
        
        caps
    }
}
