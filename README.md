# LMK Lib
A software suite for using a Raspberry Pi Zero as a smart mechanical keyboard. Used by Lynn Mechanical Keyboard, a raspberry pi zero based mechanical keyboard named after Lynn Ann Conway.

## The Suite
| Name | Readme | Description |
|------|--------|-------------|
|KServer|[link](lmk-kserver/README.md)|A server that interfaces with the Raspberry Pi's GPIO and sends mouse and key events based on hardware input to either the Pi or host. Whilst providing an interface to other programs to modify key and mouse settings, record activity, keyboard layouts, macros, and get LED state data.|
|KService|[link](lmk-kserver/README.md)|A Systemd service that setups the raspberry pi as a usb-gadget on boot.|
|HID|[link](lmk-hid/README.md)|A hid interface library.|
|Kout|[link](lmk-kout/README.md)|A command line program to convert piped in text or files to key strokes.|
|Quack|[link](lmk-quack/README.md)|A lazy ducky script interpreter that implements a subset of ducky script.|
|Bork|[link](lmk-bork/README.md)|A terse keyboard scripting language used for key and mouse automation and keystroke and mouse recording.|