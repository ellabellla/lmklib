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

## Build Environment
LmkLib is built using cross.sh. This uses docker to create a build environment used to cross compile for raspberry pi. After compilation cross.sh will transfer the build files to the specified raspberry pi remotely. SSH must be enabled on the raspberry pi.

Build the suite using: ```./cross.sh -w <REMOTE>```
Build a tool using: ```./cross.sh <REMOTE> <TARGET>```
Fix dependencies and build using: ```./cross.sh -r -f <REMOTE> <TARGET>```

#### Usage
```
Usage: ./cross.sh <REMOTE> <TARGET> 

Options:
    -r  Reload saved dependencies in build volumes before building
    -f  Fetch dependencies before building
    -w  Build the whole project (<TARGET> is ignored)
    -d  Build for debug
```

### Dependencies
- cargo
- docker
- scp

### Setup
1. Create build container - run build-cross-img.sh
2. Create required docker volumes - run create-cross-volumes.sh
3. Use the ```-r -f``` flags the first time you run cargo.sh to download required dependencies

## License
This software is provided under the MIT license. Click [here](./LICENSE) to view.
