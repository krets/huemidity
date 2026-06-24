use midir::{MidiInput, MidiInputConnection};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MidiEvent {
    pub device_name: String,
    pub event_key: String, // "CC 14" or "Note 60"
    pub value: u8,         // 0 - 127
}

pub fn get_midi_ports() -> Vec<String> {
    if let Ok(midi_in) = MidiInput::new("huemidity-input") {
        let ports = midi_in.ports();
        ports.into_iter()
            .filter_map(|port| midi_in.port_name(&port).ok())
            .collect()
    } else {
        Vec::new()
    }
}

pub struct MidiListener {
    _connection: MidiInputConnection<()>,
}

impl MidiListener {
    pub fn new<F>(port_name: &str, callback: F) -> Result<Self, Box<dyn std::error::Error>>
    where
        F: Fn(MidiEvent) + Send + 'static,
    {
        let midi_in = MidiInput::new("huemidity-input-listener")?;
        let ports = midi_in.ports();
        let port = ports.into_iter()
            .find(|p| {
                midi_in.port_name(p)
                    .map(|name| name == port_name)
                    .unwrap_or(false)
            })
            .ok_or_else(|| format!("MIDI port not found: {}", port_name))?;

        let device_name = port_name.to_string();
        let connection = midi_in.connect(&port, "huemidity-read", move |_, bytes, _| {
            if let Some((event_key, value)) = parse_midi_bytes(bytes) {
                callback(MidiEvent {
                    device_name: device_name.clone(),
                    event_key,
                    value,
                });
            }
        }, ())
        .map_err(|e| format!("MIDI connection error: {}", e))?;

        Ok(Self { _connection: connection })
    }
}

fn parse_midi_bytes(bytes: &[u8]) -> Option<(String, u8)> {
    if bytes.len() < 3 {
        return None;
    }
    let status = bytes[0];
    let control_note = bytes[1];
    let value_velocity = bytes[2];

    let message_type = status & 0xF0;
    match message_type {
        0x90 => {
            // Note On
            // If velocity is 0, it's equivalent to Note Off
            let key = format!("Note {}", control_note);
            Some((key, value_velocity))
        }
        0x80 => {
            // Note Off
            let key = format!("Note {}", control_note);
            Some((key, 0))
        }
        0xB0 => {
            // Control Change (CC)
            let key = format!("CC {}", control_note);
            Some((key, value_velocity))
        }
        _ => None,
    }
}
