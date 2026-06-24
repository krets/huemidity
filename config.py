import json
import os
import sys

CONFIG_FILE = "config.json"

def get_user_config_path(filename=CONFIG_FILE):
    if sys.platform == 'win32':
        base = os.environ.get('APPDATA') or os.path.expanduser('~\\AppData\\Roaming')
        config_dir = os.path.join(base, 'HueMIDIty')
    elif sys.platform == 'darwin':
        config_dir = os.path.expanduser('~/Library/Application Support/HueMIDIty')
    else:
        config_dir = os.path.expanduser('~/.config/huemidity')
        
    os.makedirs(config_dir, exist_ok=True)
    user_path = os.path.join(config_dir, filename)
    
    # Migrate local config if it exists and user AppData config doesn't
    local_path = os.path.join(os.path.dirname(os.path.abspath(__file__)), filename)
    if os.path.exists(local_path) and not os.path.exists(user_path):
        try:
            import shutil
            shutil.copy2(local_path, user_path)
            print(f"[Migration] Successfully migrated config from {local_path} to {user_path}")
        except Exception as e:
            print(f"[Migration] Error migrating config: {e}")
            
    return user_path

class ConfigManager:
    def __init__(self, config_file=None):
        if config_file:
            self.config_path = os.path.join(os.path.dirname(os.path.abspath(__file__)), config_file)
        else:
            self.config_path = get_user_config_path()
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

    def get_autostart(self):
        return self.data.get("autostart", True)

    def set_autostart(self, enabled):
        self.data["autostart"] = bool(enabled)
        self.save()
