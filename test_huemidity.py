import unittest
import os
import json
import time
import threading
from unittest.mock import MagicMock

# Import the modules we want to test
from config import ConfigManager
from bridge import HueRateLimiter
from midi import MidiManager

class TestHueMidity(unittest.TestCase):
    def setUp(self):
        # Create a clean test_config.json for testing
        self.test_dir = os.path.dirname(os.path.abspath(__file__))
        self.config_path = os.path.join(self.test_dir, "test_config.json")
        if os.path.exists(self.config_path):
            os.remove(self.config_path)
        self.config = ConfigManager(config_file="test_config.json")

    def tearDown(self):
        # Clean up test_config.json
        if os.path.exists(self.config_path):
            os.remove(self.config_path)

    def test_config_manager(self):
        # Verify defaults
        self.assertEqual(self.config.get_bridge_ip(), "")
        self.assertEqual(self.config.get_selected_device(), "")
        
        # Test sets & saves
        self.config.set_bridge_ip("192.168.1.50")
        self.config.set_selected_device("MidiKeyboard")
        self.assertEqual(self.config.get_bridge_ip(), "192.168.1.50")
        self.assertEqual(self.config.get_selected_device(), "MidiKeyboard")

        # Test mappings
        self.config.add_mapping("MidiKeyboard", "CC 14", "light", "1", "Brightness")
        mappings = self.config.get_mappings("MidiKeyboard")
        self.assertIn("CC 14", mappings)
        self.assertEqual(mappings["CC 14"]["action"], "Brightness")
        self.assertEqual(mappings["CC 14"]["target_id"], "1")

        self.config.remove_mapping("MidiKeyboard", "CC 14")
        self.assertNotIn("CC 14", self.config.get_mappings("MidiKeyboard"))

    def test_midi_value_scaling(self):
        # Mock dependencies
        hue_mock = MagicMock()
        midi_mgr = MidiManager(self.config, hue_mock)
        midi_mgr.selected_device = "TestController"
        
        # Add a mapping
        self.config.add_mapping("TestController", "CC 14", "light", "1", "Brightness")
        self.config.add_mapping("TestController", "Note 60", "light", "2", "Toggle On/Off")
        
        # Process midi message for CC 14 with MIDI value 127 (Max Brightness)
        midi_mgr._process_mapping("CC 14", 127)
        hue_mock.set_state.assert_called_with("light", "1", "Brightness", 254, auto_on=False)
        
        # Process midi message for CC 14 with MIDI value 0 (Min Brightness)
        midi_mgr._process_mapping("CC 14", 0)
        hue_mock.set_state.assert_called_with("light", "1", "Brightness", 0, auto_on=False)

        # Process midi message for Note 60 with MIDI value 100 (Note On triggers toggle)
        midi_mgr._process_mapping("Note 60", 100)
        hue_mock.set_state.assert_called_with("light", "2", "Toggle On/Off", "toggle", auto_on=False)

        # Test momentary toggle mappings
        self.config.add_mapping("TestController", "CC 15", "light", "2", "Toggle On/Off (Momentary)")
        hue_mock.reset_mock()
        midi_mgr._process_mapping("CC 15", 127) # Press -> toggle
        hue_mock.set_state.assert_called_with("light", "2", "Toggle On/Off (Momentary)", "toggle", auto_on=False)
        
        hue_mock.reset_mock()
        midi_mgr._process_mapping("CC 15", 0) # Release -> ignored
        hue_mock.set_state.assert_not_called()

        # Test invert mapping (0 becomes 127, 127 becomes 0)
        self.config.add_mapping("TestController", "CC 17", "light", "1", "Brightness", invert=True)
        hue_mock.reset_mock()
        midi_mgr._process_mapping("CC 17", 127)
        hue_mock.set_state.assert_called_with("light", "1", "Brightness", 0, auto_on=False)
        
        midi_mgr._process_mapping("CC 17", 0)
        hue_mock.set_state.assert_called_with("light", "1", "Brightness", 254, auto_on=False)

        # Test auto_on mapping
        self.config.add_mapping("TestController", "CC 18", "light", "1", "Brightness", auto_on=True)
        hue_mock.reset_mock()
        midi_mgr._process_mapping("CC 18", 127)
        hue_mock.set_state.assert_called_with("light", "1", "Brightness", 254, auto_on=True)
        
        # Test scene mappings
        self.config.add_mapping("TestController", "CC 16", "scene", "5/abc123xyz", "Recall Scene")
        hue_mock.reset_mock()
        midi_mgr._process_mapping("CC 16", 127) # Press -> activate scene
        hue_mock.set_scene.assert_called_with("5", "abc123xyz")

    def test_rate_limiter(self):
        bridge_mock = MagicMock()
        limiter = HueRateLimiter(bridge_mock, interval=0.05) # 50ms for faster test
        
        # Send 3 rapid brightness commands in under 5ms
        limiter.send_command("light", "1", "bri", 50)
        limiter.send_command("light", "1", "bri", 100)
        limiter.send_command("light", "1", "bri", 150)
        
        # First command should execute immediately
        bridge_mock.set_light.assert_called_once_with(1, "bri", 50)
        
        # Reset mock calls count
        bridge_mock.reset_mock()
        
        # Wait for the scheduled timer to fire (60ms)
        time.sleep(0.08)
        
        # The last value (150) should have been executed by the scheduled timer,
        # but the middle value (100) should have been skipped/throttled!
        bridge_mock.set_light.assert_called_once_with(1, "bri", 150)

if __name__ == '__main__':
    unittest.main()
