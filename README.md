# LMK Lib
A software suite for using a Raspberry Pi Zero as a smart mechanical keyboard. Used by Lynn Mechanical Keyboard, a raspberry pi zero based mechanical keyboard named after Lynn Ann Conway.

## [Docs](https://ellabellla.github.io/lmklib/target/doc/)
Rust documentation is available [here](https://ellabellla.github.io/lmklib/target/doc/).

## The Suite

### Binaries
| Name | Readme | Description |
|------|--------|-------------|
|Key-Server|[link](bin/key-server/)|A server that interfaces with the Raspberry Pi's GPIO and sends mouse and key events based on hardware input to either the Pi or host.|
|Key-Server-Cli|[link](bin/key-server-cli/)|Cli for interacting with the key server.|
|Hid-Interface|[link](bin/hid-interface/)|A fuse3 fs that creates an abstraction over hidg interfaces, allowing multiple programs to write to a hidg interface simultaneously.|
|Gadget-Service|[link](bin/gadget-service/)|A Systemd service and installer that sets up a raspberry pi as a usb-gadget on boot.|
|Kout|[link](bin/kout/)|A command line program to convert piped in text or files to key strokes.|

### Libraries
| Name | Readme | Description |
|------|--------|-------------|
|Key-Module|[link](lib/key-module/)|A library for creating key-server modules in rust and python.|
|Key-RPC|[link](lib/key-rpc/)|A library for interfacing with a key-server over nanomsg RPC.|
|mcp3008-driver|[link](lib/mcp3008-driver/)| A key-server driver module for driving a MCP3008 ic.|
|mcp23017-driver|[link](lib/mcp23017-driver/)| A key-server driver module for driving a MCP23017 ic.|
|ws-1inch5-driver|[link](lib/ws-1in5-driver/)| A key-server driver module for driving a Waveshare 1.5 Inch OLED.|

## License
This software is provided under the MIT license. Click [here](./LICENSE) to view.
