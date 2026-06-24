import os
import sys
import socket
import threading
import time
import webview

# Import our custom backend modules
from config import ConfigManager
from bridge import HueBridgeManager
from midi import MidiManager

# Constants & Globals
LOCK_PORT = 55632
lock_socket = None

config_manager = None
hue_manager = None
midi_manager = None
main_window = None
window_visible = True

# Platform-specific tray variables
win_tray_icon = None
mac_status_item = None
mac_menu_actions = None

# 1. Single Instance Lock
def acquire_single_instance_lock():
    global lock_socket
    try:
        lock_socket = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        lock_socket.bind(('127.0.0.1', LOCK_PORT))
    except socket.error:
        print("[System] Another instance of HueMIDIty is already running. Exiting.")
        sys.exit(0)

acquire_single_instance_lock()

# Change working directory to the file's folder to ensure relative paths work
BASE_DIR = os.path.dirname(os.path.abspath(__file__))
os.chdir(BASE_DIR)

# 2. Cleanup and Exit
def cleanup_and_exit():
    print("[System] Cleaning up and exiting...")
    global midi_manager, hue_manager
    try:
        if midi_manager:
            midi_manager.stop_listening()
        if hue_manager:
            hue_manager.stop_connection_loop()
    except Exception as e:
        print(f"[System] Error during cleanup: {e}")
        
    try:
        if win_tray_icon:
            win_tray_icon.stop()
    except Exception as e:
        print(f"[System] Error stopping Windows tray icon: {e}")
        
    import os
    os._exit(0)

# 3. macOS Tray Implementation using PyObjC
if sys.platform == 'darwin':
    import objc
    from Foundation import NSObject
    from AppKit import NSStatusBar, NSVariableStatusItemLength, NSMenu, NSMenuItem

    class StatusMenuActions(NSObject):
        def init(self):
            self = objc.super(StatusMenuActions, self).init()
            return self

        def toggleDashboard_(self, sender):
            global window_visible
            print("[macOS Tray] Toggle dashboard clicked.")
            if main_window:
                if window_visible:
                    main_window.hide()
                    window_visible = False
                else:
                    main_window.show()
                    main_window.focus()
                    window_visible = True

        def quitApp_(self, sender):
            print("[macOS Tray] Quit clicked.")
            cleanup_and_exit()

    def setup_macos_tray():
        global mac_status_item, mac_menu_actions
        
        status_bar = NSStatusBar.systemStatusBar()
        mac_status_item = status_bar.statusItemWithLength_(NSVariableStatusItemLength)
        
        button = mac_status_item.button()
        button.setTitle_("💡⌨️")  # Emoji icon representing light bulb + MIDI keys
        
        menu = NSMenu.alloc().init()
        mac_menu_actions = StatusMenuActions.alloc().init()
        
        dash_item = NSMenuItem.alloc().initWithTitle_action_keyEquivalent_(
            "Configuration Dashboard", "toggleDashboard:", "d"
        )
        dash_item.setTarget_(mac_menu_actions)
        menu.addItem_(dash_item)
        
        menu.addItem_(NSMenuItem.separatorItem())
        
        quit_item = NSMenuItem.alloc().initWithTitle_action_keyEquivalent_(
            "Quit", "quitApp:", "q"
        )
        quit_item.setTarget_(mac_menu_actions)
        menu.addItem_(quit_item)
        
        mac_status_item.setMenu_(menu)
        print("[macOS Tray] Native status bar item initialized.")

def set_windows_window_icon(hwnd=None):
    try:
        import ctypes
        from PIL import Image
        
        # 1. Find window handle by title
        if not hwnd:
            hwnd = ctypes.windll.user32.FindWindowW(None, "HueMIDIty Dashboard")
        if not hwnd:
            return False
            
        # 2. Save PIL image as a temp .ico file if it doesn't exist
        icon_png_path = os.path.abspath(os.path.join("ui", "icon.png"))
        temp_ico_path = os.path.abspath(os.path.join("ui", "temp_icon.ico"))
        
        if os.path.exists(icon_png_path) and not os.path.exists(temp_ico_path):
            img = Image.open(icon_png_path)
            img.save(temp_ico_path, format='ICO', sizes=[(16, 16), (32, 32), (48, 48), (64, 64)])
            
        if os.path.exists(temp_ico_path):
            # Load the icon and send WM_SETICON messages
            hicon = ctypes.windll.user32.LoadImageW(
                None, 
                temp_ico_path, 
                1, # IMAGE_ICON
                0, 0, 
                0x00000010 | 0x00008000 # LR_LOADFROMFILE | LR_SHARED
            )
            if hicon:
                # Send WM_SETICON (0x0080) for small (0) and big (1) window representations
                ctypes.windll.user32.SendMessageW(hwnd, 0x0080, 0, hicon)
                ctypes.windll.user32.SendMessageW(hwnd, 0x0080, 1, hicon)
                print("[Windows Icon] Window taskbar icon updated programmatically.")
                return True
    except Exception as e:
        print(f"[Windows Icon] Error setting window icon: {e}")
    return False

def sync_autostart(enabled):
    import sys
    import os
    try:
        if sys.platform == 'win32':
            import winreg
            key_path = r"Software\Microsoft\Windows\CurrentVersion\Run"
            key = winreg.OpenKey(winreg.HKEY_CURRENT_USER, key_path, 0, winreg.KEY_ALL_ACCESS)
            if enabled:
                executable = sys.executable
                script_path = os.path.abspath(sys.argv[0])
                venv_dir = os.path.dirname(executable)
                pythonw_path = os.path.join(venv_dir, "pythonw.exe")
                if not os.path.exists(pythonw_path):
                    pythonw_path = os.path.join(venv_dir, "python.exe")
                
                # Double quote path parameters in case there are spaces
                cmd = f'"{pythonw_path}" "{script_path}"'
                winreg.SetValueEx(key, "HueMIDIty", 0, winreg.REG_SZ, cmd)
                print(f"[Autostart] Registry run key synced: {cmd}")
            else:
                try:
                    winreg.DeleteValue(key, "HueMIDIty")
                    print("[Autostart] Registry run key deleted.")
                except FileNotFoundError:
                    pass
            winreg.CloseKey(key)
        elif sys.platform == 'darwin':
            plist_path = os.path.expanduser("~/Library/LaunchAgents/com.krets.huemidity.plist")
            if enabled:
                executable = sys.executable
                script_path = os.path.abspath(sys.argv[0])
                plist_content = f"""<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.krets.huemidity</string>
    <key>ProgramArguments</key>
    <array>
        <string>{executable}</string>
        <string>{script_path}</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
</dict>
</plist>
"""
                os.makedirs(os.path.dirname(plist_path), exist_ok=True)
                with open(plist_path, "w", encoding="utf-8") as f:
                    f.write(plist_content)
                print(f"[Autostart] macOS plist synced to {plist_path}")
            else:
                if os.path.exists(plist_path):
                    os.remove(plist_path)
                    print("[Autostart] macOS plist removed.")
    except Exception as e:
        print(f"[Autostart] Error syncing autostart: {e}")

# 4. Windows Tray Implementation using pystray
def setup_windows_tray():
    global win_tray_icon
    from PIL import Image
    
    icon_path = os.path.join("ui", "icon.png")
    try:
        image = Image.open(icon_path)
    except Exception as e:
        print(f"[Windows Tray] Could not load icon image: {e}. Generating fallback...")
        image = Image.new('RGB', (64, 64), color=(0, 240, 255))
        
    def on_toggle_clicked(icon, item):
        global window_visible
        if main_window:
            if window_visible:
                main_window.hide()
                window_visible = False
            else:
                main_window.show()
                window_visible = True
            
    def on_quit_clicked(icon, item):
        icon.stop()
        cleanup_and_exit()
        
    import pystray
    win_tray_icon = pystray.Icon(
        "HueMIDIty", 
        image, 
        menu=pystray.Menu(
            pystray.MenuItem(
                "Show/Hide Dashboard", 
                on_toggle_clicked,
                default=True
            ),
            pystray.Menu.SEPARATOR,
            pystray.MenuItem("Quit", on_quit_clicked)
        )
    )
    
    # Run in a background thread because webview starts on main thread
    tray_thread = threading.Thread(target=win_tray_icon.run, daemon=True)
    tray_thread.start()
    print("[Windows Tray] System tray icon initialized.")

# 5. JS-to-Python Webview API Bridge
class WebviewApi:
    def __init__(self, cfg, hue, midi):
        self.cfg = cfg
        self.hue = hue
        self.midi = midi

    def get_connection_status(self):
        return self.hue.status

    def get_bridge_ip(self):
        return self.hue.bridge_ip

    def connect_bridge(self, manual_ip=None):
        if manual_ip == "disconnect":
            self.hue.stop_connection_loop()
            self.hue.bridge = None
            self.hue.rate_limiter = None
            self.hue.status = 'idle'
            self.hue.bridge_ip = ''
            self.cfg.set_bridge_ip('')
            print("[API] Hue Bridge disconnected and credentials reset.")
            return True
        elif manual_ip:
            self.hue.start_connection_loop(manual_ip)
        else:
            self.hue.start_connection_loop()
        return True

    def get_lights_and_groups(self):
        return self.hue.get_lights_and_groups()

    def set_light_state(self, light_id, param, value):
        return self.hue.set_state('light', light_id, param, value)

    def set_group_state(self, group_id, param, value):
        return self.hue.set_state('group', group_id, param, value)

    def get_midi_devices(self):
        import mido
        try:
            return mido.get_input_names()
        except Exception as e:
            print(f"[API] Error getting MIDI names: {e}")
            return []

    def get_selected_midi_device(self):
        return self.midi.selected_device

    def select_midi_device(self, device_name):
        self.midi.select_device(device_name)
        return True

    def get_mappings(self):
        return self.cfg.get_mappings(self.midi.selected_device)

    def add_mapping(self, event_key, target_type, target_id, action, invert=False, auto_on=False):
        self.cfg.add_mapping(self.midi.selected_device, event_key, target_type, target_id, action, invert, auto_on)
        return True

    def remove_mapping(self, event_key):
        self.cfg.remove_mapping(self.midi.selected_device, event_key)
        return True

    def get_midi_status(self):
        return {
            "status": self.midi.status,
            "error": self.midi.error_message
        }

    def get_dashboard_layout(self):
        return self.cfg.get_dashboard_layout()

    def save_dashboard_layout(self, layout):
        self.cfg.set_dashboard_layout(layout)
        return True

    def get_config_path(self):
        return self.cfg.config_path

    def get_autostart(self):
        return self.cfg.get_autostart()

    def set_autostart(self, enabled):
        self.cfg.set_autostart(enabled)
        sync_autostart(enabled)
        return True

    def quit_application(self):
        print("[API] Quit application requested via UI.")
        threading.Thread(target=cleanup_and_exit, daemon=True).start()
        return True

# 6. Main Runner
def main():
    global config_manager, hue_manager, midi_manager, main_window, window_visible
    
    if sys.platform == 'win32':
        try:
            import ctypes
            myappid = 'krets.huemidity.app'
            ctypes.windll.shell32.SetCurrentProcessExplicitAppUserModelID(myappid)
            print(f"[Windows Icon] AppUserModelID set to {myappid}")
        except Exception as e:
            print(f"[Windows Icon] Error setting AppUserModelID at startup: {e}")
            
    print("[System] Initializing Managers...")
    config_manager = ConfigManager()
    sync_autostart(config_manager.get_autostart())
    hue_manager = HueBridgeManager(config_manager)
    midi_manager = MidiManager(config_manager, hue_manager)
    
    api = WebviewApi(config_manager, hue_manager, midi_manager)
    
    # Establish dynamic callback pushing MIDI activity to UI
    def push_midi_to_ui(event_key, value, learn_cache):
        if main_window:
            try:
                import json
                js = f"if (window.onMidiActivity) {{ window.onMidiActivity({json.dumps(event_key)}, {value}, {json.dumps(learn_cache)}); }}"
                main_window.evaluate_js(js)
            except Exception as e:
                pass
                
    midi_manager.set_ui_callback(push_midi_to_ui)
    
    # Create window (always visible on start to guide user, hides on click-off if supported)
    main_window = webview.create_window(
        'HueMIDIty Dashboard', 
        'ui/index.html', 
        width=950, 
        height=680, 
        js_api=api,
        min_size=(900, 600)
    )
    
    def on_closing():
        global window_visible
        if sys.platform in ('darwin', 'win32'):
            main_window.hide()
            window_visible = False
            print("[System] Window closed. Running in system tray.")
            return False  # Abort closing, hide instead
        return True  # Normal close on unsupported platforms

    main_window.events.closing += on_closing
    
    def on_webview_start():
        global window_visible
        window_visible = True
        
        # Load system tray based on platform
        if sys.platform == 'darwin':
            setup_macos_tray()
            # Set Dock Icon
            try:
                from AppKit import NSApplication, NSImage
                icon_path = os.path.abspath("ui/icon.png")
                image = NSImage.alloc().initByReferencingFile_(icon_path)
                NSApplication.sharedApplication().setApplicationIconImage_(image)
            except Exception as e:
                print(f"[macOS Icon] Error setting Dock icon: {e}")
                
        elif sys.platform == 'win32':
            setup_windows_tray()
            # Set taskbar window icon programmatically with retries to ensure window creation is complete
            def apply_win_icon():
                for _ in range(50): # Try for 5 seconds
                    time.sleep(0.1)
                    if set_windows_window_icon():
                        break
            threading.Thread(target=apply_win_icon, daemon=True).start()

    webview.start(on_webview_start)

if __name__ == '__main__':
    main()
