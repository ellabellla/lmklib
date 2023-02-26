# Hid-Interface
A fuse3 fs that creates an abstraction over hidg interfaces, allowing multiple programs to write to a hidg interface simultaneously.

## Usage
```
Usage: hid-interface <MOUNT> [KEYBOARD] [MOUSE]

Arguments:
  <MOUNT>     Mount point
  [KEYBOARD]  Keyboard Interface Path (Default: /dev/hidg1)
  [MOUSE]     Mouse Interface Path (Default: /dev/hidg0)

Options:
  -h, --help  Print help information
```