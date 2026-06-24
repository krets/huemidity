import threading
import time
import requests
import phue

class HueRateLimiter:
    def __init__(self, bridge, interval=0.1):
        self.bridge = bridge
        self.interval = interval  # 100ms throttle
        self.lock = threading.Lock()
        self.pending = {}  # key: (target_type, target_id, param) -> value
        self.last_sent_time = {}  # key: (target_type, target_id, param) -> timestamp
        self.timers = {}  # key: (target_type, target_id, param) -> Timer object

    def send_command(self, target_type, target_id, param, value):
        key = (target_type, target_id, param)
        with self.lock:
            now = time.time()
            last_time = self.last_sent_time.get(key, 0)
            
            # Cancel any scheduled execution since we have a new value
            if key in self.timers:
                self.timers[key].cancel()
                del self.timers[key]
            
            if now - last_time >= self.interval:
                # Execute immediately
                self._execute(target_type, target_id, param, value)
                self.last_sent_time[key] = now
            else:
                # Schedule execution of the latest value
                self.pending[key] = value
                delay = self.interval - (now - last_time)
                t = threading.Timer(delay, self._execute_pending, [key])
                self.timers[key] = t
                t.start()

    def _execute_pending(self, key):
        with self.lock:
            if key in self.pending:
                value = self.pending.pop(key)
                if key in self.timers:
                    del self.timers[key]
                target_type, target_id, param = key
                self._execute(target_type, target_id, param, value)
                self.last_sent_time[key] = time.time()

    def _execute(self, target_type, target_id, param, value):
        try:
            tid = int(target_id)
            if target_type == 'light':
                self.bridge.set_light(tid, param, value)
            elif target_type == 'group':
                self.bridge.set_group(tid, param, value)
        except Exception as e:
            print(f"[Limiter] Error setting {target_type} {target_id} {param} to {value}: {e}")

class HueBridgeManager:
    def __init__(self, config_manager):
        self.config_manager = config_manager
        self.bridge = None
        self.rate_limiter = None
        
        # Statuses: 'idle', 'searching', 'needs_link', 'connected', 'error'
        self.status = 'idle'
        self.error_message = ''
        self.bridge_ip = self.config_manager.get_bridge_ip()
        
        self.conn_thread = None
        self.stop_conn_event = threading.Event()
        
        # Start connection loop (connect to saved IP or auto-discover if empty)
        self.start_connection_loop()

    def discover_bridge_ip(self):
        try:
            self.status = 'searching'
            print("[Hue] Discovering Hue Bridge IP...")
            r = requests.get('https://discovery.meethue.com/', timeout=5)
            data = r.json()
            if data and isinstance(data, list) and len(data) > 0:
                ip = data[0].get('internalipaddress')
                if ip:
                    self.bridge_ip = ip
                    self.config_manager.set_bridge_ip(ip)
                    print(f"[Hue] Discovered bridge IP: {ip}")
                    return ip
        except Exception as e:
            print(f"[Hue] Discovery error: {e}")
        self.status = 'error'
        self.error_message = "Could not discover Hue Bridge automatically."
        return None

    def start_connection_loop(self, manual_ip=None):
        if manual_ip:
            self.bridge_ip = manual_ip
            self.config_manager.set_bridge_ip(manual_ip)
            
        self.stop_connection_loop()
        self.stop_conn_event.clear()
        self.status = 'connecting'
        self.conn_thread = threading.Thread(target=self._connection_loop, daemon=True)
        self.conn_thread.start()

    def stop_connection_loop(self):
        self.stop_conn_event.set()
        if self.conn_thread and self.conn_thread.is_alive():
            self.conn_thread.join(timeout=1.0)

    def _connection_loop(self):
        if not self.bridge_ip:
            ip = self.discover_bridge_ip()
            if not ip:
                self.status = 'error'
                self.error_message = "Bridge IP not found. Please enter manually."
                return

        while not self.stop_conn_event.is_set():
            try:
                print(f"[Hue] Connecting to Hue Bridge at {self.bridge_ip}...")
                b = phue.Bridge(self.bridge_ip)
                b.connect()
                
                # Successful connection!
                self.bridge = b
                self.rate_limiter = HueRateLimiter(b)
                self.status = 'connected'
                print("[Hue] Successfully connected to Hue Bridge.")
                break
            except phue.PhueRegistrationException:
                self.status = 'needs_link'
                self.error_message = "Press the link button on your Hue Bridge."
                print("[Hue] Link button not pressed. Retrying in 3 seconds...")
            except Exception as e:
                self.status = 'error'
                self.error_message = f"Connection failed: {str(e)}"
                print(f"[Hue] Connection error: {e}. Retrying in 3 seconds...")
            
            # Interruptible sleep for 3 seconds
            for _ in range(30):
                if self.stop_conn_event.is_set():
                    return
                time.sleep(0.1)

    def get_lights_and_groups(self):
        if self.status != 'connected' or not self.bridge:
            return {'lights': [], 'groups': []}
            
        try:
            lights_data = self.bridge.get_light()
            groups_data = self.bridge.get_group()
            
            lights = []
            if isinstance(lights_data, dict):
                for lid, ldata in lights_data.items():
                    state = ldata.get('state', {})
                    lights.append({
                        'id': str(lid),
                        'name': ldata.get('name', f"Light {lid}"),
                        'on': state.get('on', False),
                        'bri': state.get('bri', 0),
                        'hue': state.get('hue', 0),
                        'sat': state.get('sat', 0)
                    })
            
            groups = []
            if isinstance(groups_data, dict):
                for gid, gdata in groups_data.items():
                    action = gdata.get('action', {})
                    groups.append({
                        'id': str(gid),
                        'name': gdata.get('name', f"Group {gid}"),
                        'on': action.get('on', False),
                        'bri': action.get('bri', 0),
                        'hue': action.get('hue', 0),
                        'sat': action.get('sat', 0)
                    })
                    
            return {'lights': lights, 'groups': groups}
        except Exception as e:
            print(f"[Hue] Error fetching devices: {e}")
            return {'lights': [], 'groups': []}

    def set_state(self, target_type, target_id, param, value):
        if self.status != 'connected' or not self.rate_limiter or not self.bridge:
            return False
            
        # Map parameters from JS UI / MIDI actions to phue attributes
        hue_param = None
        if param in ('Toggle On/Off', 'Toggle On/Off (Latch)', 'Toggle On/Off (Momentary)', 'on'):
            hue_param = 'on'
        elif param in ('Brightness', 'bri', 'Value'):
            hue_param = 'bri'
        elif param in ('Hue', 'hue'):
            hue_param = 'hue'
        elif param in ('Saturation', 'sat'):
            hue_param = 'sat'
            
        if not hue_param:
            return False
            
        # Toggle logic if value is None or 'toggle'
        if hue_param == 'on' and (value is None or value == 'toggle'):
            try:
                if target_type == 'light':
                    current_on = self.bridge.get_light(int(target_id), 'on')
                elif target_type == 'group':
                    group_data = self.bridge.get_group(int(target_id))
                    current_on = group_data.get('action', {}).get('on', False)
                value = not current_on
            except Exception as e:
                print(f"[Hue] Toggle state read error: {e}")
                value = True  # fallback
                
        self.rate_limiter.send_command(target_type, target_id, hue_param, value)
        return True
