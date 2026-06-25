pub mod types;

use types::{Light, Group, Scene};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;
use reqwest::Client;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum BridgeConnectionState {
    Idle,
    Searching,
    NeedsLink { ip: String, countdown: u32 },
    Connected { ip: String, username: String },
    Error(String),
}

#[derive(Deserialize, Debug)]
struct MeetHueResponse {
    internalipaddress: String,
}

#[derive(Deserialize, Debug)]
struct HueApiResponseItem {
    success: Option<HueApiSuccess>,
    error: Option<HueApiError>,
}

#[derive(Deserialize, Debug)]
struct HueApiSuccess {
    username: String,
}

#[derive(Deserialize, Debug)]
struct HueApiError {
    r#type: u32,
    description: String,
}

#[derive(Clone)]
pub struct HueClient {
    client: Client,
}

impl HueClient {
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(3))
            .build()
            .unwrap_or_default();
        Self { client }
    }

    /// Queries meethue discovery service to find local Hue Bridges
    pub async fn discover_bridges(&self) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
        let url = "https://discovery.meethue.com/";
        let resp = self.client.get(url).send().await?;
        let bridges: Vec<MeetHueResponse> = resp.json().await?;
        Ok(bridges.into_iter().map(|b| b.internalipaddress).collect())
    }

    /// Attempts to register developer key by polling the link button.
    /// Returns Ok(Some(username)) if registered, Ok(None) if link button is not pressed, or Error.
    pub async fn register_app(&self, ip: &str) -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("http://{}/api", ip);
        let body = serde_json::json!({
            "devicetype": "huemidity#app"
        });

        let resp = self.client.post(&url).json(&body).send().await?;
        let results: Vec<HueApiResponseItem> = resp.json().await?;

        for item in results {
            if let Some(success) = item.success {
                return Ok(Some(success.username));
            }
            if let Some(error) = item.error {
                // Error type 101 is "link button not pressed"
                if error.r#type == 101 {
                    return Ok(None);
                } else {
                    return Err(error.description.into());
                }
            }
        }

        Ok(None)
    }

    pub async fn fetch_lights(&self, ip: &str, username: &str) -> Result<HashMap<String, Light>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("http://{}/api/{}/lights", ip, username);
        let resp = self.client.get(&url).send().await?;
        let lights: HashMap<String, Light> = resp.json().await?;
        Ok(lights)
    }

    pub async fn fetch_groups(&self, ip: &str, username: &str) -> Result<HashMap<String, Group>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("http://{}/api/{}/groups", ip, username);
        let resp = self.client.get(&url).send().await?;
        let groups: HashMap<String, Group> = resp.json().await?;
        Ok(groups)
    }

    pub async fn fetch_scenes(&self, ip: &str, username: &str) -> Result<HashMap<String, Scene>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("http://{}/api/{}/scenes", ip, username);
        let resp = self.client.get(&url).send().await?;
        let scenes: HashMap<String, Scene> = resp.json().await?;
        Ok(scenes)
    }

    pub async fn set_light_state(&self, ip: &str, username: &str, light_id: &str, body: &Value) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("http://{}/api/{}/lights/{}/state", ip, username, light_id);
        self.client.put(&url).json(body).send().await?;
        Ok(())
    }

    pub async fn set_group_action(&self, ip: &str, username: &str, group_id: &str, body: &Value) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("http://{}/api/{}/groups/{}/action", ip, username, group_id);
        self.client.put(&url).json(body).send().await?;
        Ok(())
    }
}
