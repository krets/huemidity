import ctypes
from ctypes import wintypes
import threading
import os

# Win32 Constants
WM_USER = 0x0400
WM_TRAYMESSAGE = WM_USER + 20

# Mouse messages
WM_LBUTTONUP = 0x0202
WM_LBUTTONDBLCLK = 0x0203
WM_RBUTTONUP = 0x0205
WM_CONTEXTMENU = 0x007B

# Shell_NotifyIcon actions
NIM_ADD = 0x00000000
NIM_MODIFY = 0x00000001
NIM_DELETE = 0x00000002

# NOTIFYICONDATA flags
NIF_MESSAGE = 0x00000001
NIF_ICON = 0x00000002
NIF_TIP = 0x00000004

# Menu flags
MF_STRING = 0x00000000
MF_SEPARATOR = 0x00000800
TPM_LEFTALIGN = 0x0000
TPM_RIGHTBUTTON = 0x0002
TPM_RETURNCMD = 0x0100

# Window Messages
WM_DESTROY = 0x0002

# Load User32, Shell32, Kernel32
user32 = ctypes.windll.user32
shell32 = ctypes.windll.shell32
kernel32 = ctypes.windll.kernel32

class POINT(ctypes.Structure):
    _fields_ = [("x", wintypes.LONG), ("y", wintypes.LONG)]

class NOTIFYICONDATAW(ctypes.Structure):
    _fields_ = [
        ("cbSize", wintypes.DWORD),
        ("hWnd", wintypes.HWND),
        ("uID", wintypes.UINT),
        ("uFlags", wintypes.UINT),
        ("uCallbackMessage", wintypes.UINT),
        ("hIcon", wintypes.HICON),
        ("szTip", ctypes.c_wchar * 128),
        ("dwState", wintypes.DWORD),
        ("dwStateMask", wintypes.DWORD),
        ("szInfo", ctypes.c_wchar * 256),
        ("uTimeoutOrVersion", wintypes.UINT),
        ("szInfoTitle", ctypes.c_wchar * 64),
        ("dwInfoFlags", wintypes.DWORD),
        ("guidItem", ctypes.c_byte * 16),
        ("hBalloonIcon", wintypes.HICON)
    ]

# Define WNDPROC
WNDPROC = ctypes.WINFUNCTYPE(ctypes.c_int64, wintypes.HWND, wintypes.UINT, wintypes.WPARAM, wintypes.LPARAM)

class WNDCLASSW(ctypes.Structure):
    _fields_ = [
        ("style", wintypes.UINT),
        ("lpfnWndProc", WNDPROC),
        ("cbClsExtra", ctypes.c_int),
        ("cbWndExtra", ctypes.c_int),
        ("hInstance", wintypes.HINSTANCE),
        ("hIcon", wintypes.HICON),
        ("hCursor", wintypes.HICON),
        ("hbrBackground", wintypes.HBRUSH),
        ("lpszMenuName", wintypes.LPCWSTR),
        ("lpszClassName", wintypes.LPCWSTR)
    ]

# Declare API signatures for 64-bit safety
kernel32.GetModuleHandleW.argtypes = [wintypes.LPCWSTR]
kernel32.GetModuleHandleW.restype = wintypes.HINSTANCE

user32.RegisterClassW.argtypes = [ctypes.POINTER(WNDCLASSW)]
user32.RegisterClassW.restype = wintypes.ATOM

user32.CreateWindowExW.argtypes = [
    wintypes.DWORD, wintypes.LPCWSTR, wintypes.LPCWSTR, wintypes.DWORD,
    ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_int,
    wintypes.HWND, wintypes.HMENU, wintypes.HINSTANCE, wintypes.LPVOID
]
user32.CreateWindowExW.restype = wintypes.HWND

user32.UnregisterClassW.argtypes = [wintypes.LPCWSTR, wintypes.HINSTANCE]
user32.UnregisterClassW.restype = wintypes.BOOL

user32.LoadImageW.argtypes = [
    wintypes.HINSTANCE, wintypes.LPCWSTR, wintypes.UINT,
    ctypes.c_int, ctypes.c_int, wintypes.UINT
]
user32.LoadImageW.restype = wintypes.HANDLE

shell32.Shell_NotifyIconW.argtypes = [wintypes.DWORD, ctypes.POINTER(NOTIFYICONDATAW)]
shell32.Shell_NotifyIconW.restype = wintypes.BOOL

user32.PostMessageW.argtypes = [wintypes.HWND, wintypes.UINT, wintypes.WPARAM, wintypes.LPARAM]
user32.PostMessageW.restype = wintypes.BOOL

user32.DefWindowProcW.argtypes = [wintypes.HWND, wintypes.UINT, wintypes.WPARAM, wintypes.LPARAM]
user32.DefWindowProcW.restype = ctypes.c_int64

user32.GetMessageW.argtypes = [ctypes.POINTER(wintypes.MSG), wintypes.HWND, wintypes.UINT, wintypes.UINT]
user32.GetMessageW.restype = wintypes.BOOL

user32.TranslateMessage.argtypes = [ctypes.POINTER(wintypes.MSG)]
user32.TranslateMessage.restype = wintypes.BOOL

user32.DispatchMessageW.argtypes = [ctypes.POINTER(wintypes.MSG)]
user32.DispatchMessageW.restype = wintypes.LPARAM

user32.PostQuitMessage.argtypes = [ctypes.c_int]
user32.PostQuitMessage.restype = None

user32.CreatePopupMenu.argtypes = []
user32.CreatePopupMenu.restype = wintypes.HMENU

user32.AppendMenuW.argtypes = [wintypes.HMENU, wintypes.UINT, ctypes.c_uint64, wintypes.LPCWSTR]
user32.AppendMenuW.restype = wintypes.BOOL

user32.GetCursorPos.argtypes = [ctypes.POINTER(POINT)]
user32.GetCursorPos.restype = wintypes.BOOL

user32.SetForegroundWindow.argtypes = [wintypes.HWND]
user32.SetForegroundWindow.restype = wintypes.BOOL

user32.TrackPopupMenu.argtypes = [
    wintypes.HMENU, wintypes.UINT, ctypes.c_int, ctypes.c_int, ctypes.c_int,
    wintypes.HWND, ctypes.c_void_p
]
user32.TrackPopupMenu.restype = wintypes.BOOL

user32.DestroyMenu.argtypes = [wintypes.HMENU]
user32.DestroyMenu.restype = wintypes.BOOL

class WindowsTrayIcon:
    def __init__(self, icon_path, tooltip, on_toggle, on_quit):
        self.icon_path = os.path.abspath(icon_path)
        self.tooltip = tooltip
        self.on_toggle = on_toggle
        self.on_quit = on_quit
        
        self.hwnd = None
        self.thread = None
        self.running = False
        
        # Load the .ico file
        self.hicon = user32.LoadImageW(
            None,
            self.icon_path,
            1, # IMAGE_ICON
            0, 0,
            0x00000010 | 0x00008000 # LR_LOADFROMFILE | LR_SHARED
        )
        if not self.hicon:
            print(f"[Tray] Failed to load icon: {self.icon_path}")

    def start(self):
        self.running = True
        self.thread = threading.Thread(target=self._run, daemon=True)
        self.thread.start()

    def stop(self):
        self.running = False
        if self.hwnd:
            user32.PostMessageW(self.hwnd, WM_DESTROY, 0, 0)
        if self.thread:
            self.thread.join(timeout=2.0)

    def _run(self):
        class_name = "HueMidityTrayWindow"
        
        # Keep reference to callback to prevent garbage collection
        self.wndproc_cb = WNDPROC(self.wnd_proc)
        
        wc = WNDCLASSW()
        wc.hInstance = kernel32.GetModuleHandleW(None)
        wc.lpszClassName = class_name
        wc.lpfnWndProc = self.wndproc_cb
        
        user32.RegisterClassW(ctypes.byref(wc))
        
        # Create message-only window
        hwnd_message = wintypes.HWND(-3)
        self.hwnd = user32.CreateWindowExW(
            0, class_name, "TrayListener",
            0, 0, 0, 0, 0,
            hwnd_message,
            None, wc.hInstance, None
        )
        
        if not self.hwnd:
            print("[Tray] Failed to create hidden window.")
            return

        # Add tray icon
        self.nid = NOTIFYICONDATAW()
        self.nid.cbSize = ctypes.sizeof(NOTIFYICONDATAW)
        self.nid.hWnd = self.hwnd
        self.nid.uID = 1
        self.nid.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP
        self.nid.uCallbackMessage = WM_TRAYMESSAGE
        self.nid.hIcon = self.hicon
        self.nid.szTip = self.tooltip
        
        shell32.Shell_NotifyIconW(NIM_ADD, ctypes.byref(self.nid))
        print("[Tray] Windows system tray icon added.")

        # Message loop
        msg = wintypes.MSG()
        while user32.GetMessageW(ctypes.byref(msg), None, 0, 0) > 0:
            user32.TranslateMessage(ctypes.byref(msg))
            user32.DispatchMessageW(ctypes.byref(msg))
            
        # Clean up tray icon
        shell32.Shell_NotifyIconW(NIM_DELETE, ctypes.byref(self.nid))
        user32.UnregisterClassW(class_name, wc.hInstance)
        print("[Tray] Windows system tray icon removed.")

    def wnd_proc(self, hwnd, msg, wparam, lparam):
        if msg == WM_DESTROY:
            user32.PostQuitMessage(0)
            return 0
            
        elif msg == WM_TRAYMESSAGE:
            if lparam in (WM_LBUTTONUP, WM_LBUTTONDBLCLK):
                self.on_toggle(None, None) # Match signature or call standard callable
                return 0
                
            elif lparam == WM_RBUTTONUP:
                self.show_context_menu()
                return 0
                
        return user32.DefWindowProcW(hwnd, msg, wparam, lparam)

    def show_context_menu(self):
        # 1. Create Popup Menu
        menu = user32.CreatePopupMenu()
        user32.AppendMenuW(menu, MF_STRING, 1, "Show/Hide Dashboard")
        user32.AppendMenuW(menu, MF_SEPARATOR, 0, None)
        user32.AppendMenuW(menu, MF_STRING, 2, "Quit")
        
        # 2. Get mouse position
        pos = POINT()
        user32.GetCursorPos(ctypes.byref(pos))
        
        # 3. Set foreground window so clicking away closes the menu
        user32.SetForegroundWindow(self.hwnd)
        
        # 4. Show the menu and get selection
        cmd = user32.TrackPopupMenu(
            menu,
            TPM_LEFTALIGN | TPM_RIGHTBUTTON | TPM_RETURNCMD,
            pos.x, pos.y, 0,
            self.hwnd, None
        )
        
        # 5. Clean up menu handle
        user32.DestroyMenu(menu)
        
        # 6. Execute action
        if cmd == 1:
            self.on_toggle(None, None)
        elif cmd == 2:
            self.on_quit(None, None)
