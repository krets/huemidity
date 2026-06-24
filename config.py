import json
import os

CONFIG_FILE = "config.json"

class ConfigManager:
    def __init__(self, config_file=None):
        # Resolve config.json relative to this file
        filename = config_file or CONFIG_FILE
        self.config_path = os.path.join(os.path.dirname(os.path.abspath(__file__)), filename)
        self.data = self.load()

    def load(self):
        if os.path.exists(self.config_path):
            try:
                with open(self.config_path, 'r', encoding='utf-8') as f:
                    return json.load(f)
            except Exception as e:
                print(f"Error loading config.json: {e}")
        return {
            "bridge_ip": "",
            "selected_device": "",
            "mappings": {}  # device_name -> { event_key -> {target_type, target_id, action} }
        }

    def save(self):
        try:
            with open(self.config_path, 'w', encoding='utf-8') as f:
                json.dump(self.data, f, indent=4)
        except Exception as e:
            print(f"Error saving config.json: {e}")

    def get_bridge_ip(self):
        return self.data.get("bridge_ip", "")

    def set_bridge_ip(self, ip):
        self.data["bridge_ip"] = ip
        self.save()

    def get_selected_device(self):
        return self.data.get("selected_device", "")

    def set_selected_device(self, device):
        self.data["selected_device"] = device
        self.save()

    def get_mappings(self, device_name):
        if not device_name:
            return {}
        return self.data.get("mappings", {}).get(device_name, {})

    def add_mapping(self, device_name, event_key, target_type, target_id, action, invert=False, auto_on=False):
        if not device_name:
            return
        if "mappings" not in self.data:
            self.data["mappings"] = {}
        if device_name not in self.data["mappings"]:
            self.data["mappings"][device_name] = {}
        self.data["mappings"][device_name][event_key] = {
            "target_type": target_type,
            "target_id": str(target_id),
            "action": action,
            "invert": bool(invert),
            "auto_on": bool(auto_on)
        }
        self.save()

    def remove_mapping(self, device_name, event_key):
        if not device_name:
            return
        if "mappings" in self.data and device_name in self.data["mappings"]:
            if event_key in self.data["mappings"][device_name]:
                del self.data["mappings"][device_name][event_key]
                self.save()

    def get_dashboard_layout(self):
        return self.data.get("dashboard_layout", [])

    def set_dashboard_layout(self, layout):
        self.data["dashboard_layout"] = layout
        self.save()
