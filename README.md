# HueMIDIty

This tool was made entirely for a friend, Giovanni, to have a Mac capable Philips Hue interface.

HueMIDIty is a basic desktop tool to interact with a Philips Hue Bridge. Additionally, it can listen to MIDI devices and map them to light functions like brightness, hue, or component RGB.

## Download & Releases

Download precompiled binaries for macOS and Windows from the [Releases](https://github.com/krets/huemidity/releases) page.

## Security Warnings

The macOS and Windows builds are not code-signed, so your OS will likely warn you before letting the app run:

- **macOS**: Gatekeeper will say the app is from an unidentified developer. Right-click (or Control-click) the app and choose "Open" to bypass this.
- **Windows**: SmartScreen may warn that the app is unrecognized. Click "More info" then "Run anyway".

On macOS, the app also needs **Local Network** access to find and talk to your Hue Bridge. Approve the local network permission prompt when asked (or enable it under System Settings > Privacy & Security > Local Network).

## Support

If you run into issues, have feedback, or want to submit support requests, please open an issue on GitHub at:
[https://github.com/krets/huemidity/issues](https://github.com/krets/huemidity/issues)

## License

This project is licensed under the [MIT License](LICENSE).
