import threading
import mido
import time

class MidiManager:
    def __init__(self, config_manager, hue_manager, ui_callback=None):
        self.config_manager = config_manager
        self.hue_manager = hue_manager
        self.ui_callback = ui_callback
        
        self.selected_device = self.config_manager.get_selected_device()
        self.learn_cache = []  # rolling cache of last 10: [{'key': 'CC 14', 'value': 64}, ...]
        self.cache_lock = threading.Lock()
        
        self.status = 'disconnected'  # 'disconnected', 'listening', 'error'
        self.error_message = ''
        
        self.thread = None
        self.stop_event = threading.Event()
        self.port = None
        
        # Auto-start if a device was previously selected
        if self.selected_device:
            self.start_listening()

    def set_ui_callback(self, callback):
        self.ui_callback = callback

    def get_learn_cache(self):
        with self.cache_lock:
            return list(self.learn_cache)

    def add_to_learn_cache(self, event_key, value):
        with self.cache_lock:
            # Remove previous occurrences of this event key to move it to the top
            self.learn_cache = [e for e in self.learn_cache if e['key'] != event_key]
            timestamp = time.strftime('%Y-%m-%d %H:%M:%S')
            self.learn_cache.insert(0, {'key': event_key, 'value': value, 'time': timestamp})
            self.learn_cache = self.learn_cache[:10]

    def select_device(self, device_name):
        print(f"[MIDI] Selecting device: {device_name}")
        self.stop_listening()
        self.selected_device = device_name
        self.config_manager.set_selected_device(device_name)
        if device_name:
            self.start_listening()

    def start_listening(self):
        self.stop_listening()
        self.stop_event.clear()
        
        if not self.selected_device:
            self.status = 'disconnected'
            self.error_message = ''
            self.trigger_status_update()
            return
            
        available = mido.get_input_names()
        matching_port = None
        for p in available:
            if self.selected_device in p:
                matching_port = p
                break
                
        if not matching_port:
            self.status = 'error'
            self.error_message = f"Device '{self.selected_device}' is not currently available."
            print(f"[MIDI] {self.error_message}")
            self.trigger_status_update()
            return
            
        self.status = 'connecting'
        self.thread = threading.Thread(
            target=self._listen_loop, 
            args=(matching_port,), 
            name="MidiListenerThread", 
            daemon=True
        )
        self.thread.start()
        print(f"[MIDI] Started listening thread on port: {matching_port}")

    def trigger_status_update(self):
        if self.ui_callback:
            try:
                # Push None event key to signal a status/cache refresh
                self.ui_callback(None, None, self.get_learn_cache())
            except Exception as e:
                pass

    def stop_listening(self):
        self.stop_event.set()
        
        if self.port:
            try:
                self.port.close()
            except Exception as e:
                print(f"[MIDI] Error closing port: {e}")
            self.port = None
            
        if self.thread and self.thread.is_alive():
            self.thread.join(timeout=1.0)
        self.thread = None

    def _listen_loop(self, port_name):
        try:
            self.port = mido.open_input(port_name)
            self.status = 'listening'
            self.error_message = ''
            self.trigger_status_update()
            
            # Using the port generator which unblocks when port.close() is called
            for msg in self.port:
                if self.stop_event.is_set():
                    break
                    
                event_key = None
                value = 0
                
                if msg.type == 'control_change':
                    event_key = f"CC {msg.control}"
                    value = msg.value
                elif msg.type == 'note_on':
                    event_key = f"Note {msg.note}"
                    value = msg.velocity
                elif msg.type == 'note_off':
                    event_key = f"Note {msg.note}"
                    value = 0
                    
                if event_key:
                    # Update cache
                    self.add_to_learn_cache(event_key, value)
                    
                    # Notify UI
                    if self.ui_callback:
                        try:
                            self.ui_callback(event_key, value, self.get_learn_cache())
                        except Exception as e:
                            # Catch potential thread issues if UI is closed or reloaded
                            pass
                        
                    # Execute mapped command
                    self._process_mapping(event_key, value)
                    
        except Exception as e:
            self.status = 'error'
            self.error_message = str(e)
            print(f"[MIDI] Error in listen loop: {e}")
            self.trigger_status_update()
        finally:
            self.port = None
            if self.status == 'listening':
                self.status = 'disconnected'
            self.trigger_status_update()
            print("[MIDI] Listen loop stopped.")

    def _process_mapping(self, event_key, value):
        if not self.selected_device:
            return
            
        mappings = self.config_manager.get_mappings(self.selected_device)
        if event_key not in mappings:
            return
            
        mapping = mappings[event_key]
        target_type = mapping.get('target_type')
        target_id = mapping.get('target_id')
        if target_type == 'scene':
            if '/' in target_id:
                group_id, scene_id = target_id.split('/', 1)
                is_press = False
                if "Note" in event_key:
                    is_press = (value > 0)
                else:
                    is_press = (value >= 64)
                if is_press:
                    self.hue_manager.set_scene(group_id, scene_id)
            return

        action = mapping.get('action')
        hue_value = None
        if action in ('Toggle On/Off', 'Toggle On/Off (Latch)', 'Toggle On/Off (Momentary)'):
            if action == 'Toggle On/Off (Momentary)':
                if "Note" in event_key:
                    if value > 0:
                        hue_value = 'toggle'
                    else:
                        return  # ignore note release
                else:
                    # CC Momentary: Toggle only on press (>= 64)
                    if value >= 64:
                        hue_value = 'toggle'
                    else:
                        return  # ignore release
            else:
                # Latch or legacy Toggle On/Off
                if "Note" in event_key:
                    if value > 0:
                        hue_value = 'toggle'
                    else:
                        return
                else:
                    # CC toggle: value >= 64 is ON, < 64 is OFF
                    hue_value = True if value >= 64 else False
        elif action in ('Brightness', 'Value'):
            # Scale 0-127 -> 0-254
            hue_value = int(value * (254.0 / 127.0))
        elif action == 'Hue':
            # Scale 0-127 -> 0-65535
            hue_value = int(value * (65535.0 / 127.0))
        elif action == 'Saturation':
            # Scale 0-127 -> 0-254
            hue_value = int(value * (254.0 / 127.0))
            
        if hue_value is not None:
            self.hue_manager.set_state(target_type, target_id, action, hue_value)
