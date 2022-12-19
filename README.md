# LMK Lib
A software suite for using a Raspberry Pi Zero as a smart mechanical keyboard. Used by Lynn Mechanical Keyboard, a raspberry pi zero based mechanical keyboard named after Lynn Ann Conway.

## The Suite
| Name | Readme | Description |
|------|--------|-------------|
|Key-Server|[link](bin/kserver/)|A server that interfaces with the Raspberry Pi's GPIO and sends mouse and key events based on hardware input to either the Pi or host.|
|Gadget-Service|[link](bin/gadget-service/)|A Systemd service and installer that sets up a raspberry pi as a usb-gadget on boot.|
|Virt-HID|[link](lib/virt-hid/)|A hid interface library.|
|Kout|[link](bin/kout/)|A command line program to convert piped in text or files to key strokes.|
|Quack|[link](bin/quack/)|A lazy ducky script interpreter that implements a subset of ducky script.|
|Bork|[link](bin/bork/)|A terse keyboard scripting language used for key and mouse automation and keystroke and mouse recording.|
