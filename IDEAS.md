# Lynn mechanical keyboard Library (LMKLib)
A software suite for using a raspberry pi zero as a smart mechanical keyboard. Used by Lynn Keeb a raspberry pi zero based mechanical keyboard named after Lynn Ann Conway.

# installer
- setup usb gadget
- install software

# hid
- library for hid setup and use

# k-server
- Handles input from GPIO
  - uses layouts to convert switch presses to keystrokes
  - controls whether input should be sent to the keyboard or the host
  - controls mouse input from GPIO
- api
  - layouts
    - see active layout
    - see all available layouts
    - switch layouts
  - change/see output mode
    - keyboard
    - host
  - input recording
    - send
    - record
  - get host info

# settings
- database service
  - stores
    - macros
    - layouts
    - mouse settings
  - provides key-value store

# settings-manager
- provides frontend for settings database

# backup
- backup service
  - backups all lmklib data
  - backups user specified data

# layout
- tui/cli interface for k-server layout api
  - see active layout
  - see all available layouts
  - switch layouts
- modes
  - tui mode
    - graphical
    - shows current layout in a visual way
    - allows simple settings changes
  - cli mode
    - allows granular settings changes 

# quack
- run Ducky script files
- will support a subset of the full spec

# kout
- pipes input from stdin to the keyboard out

# macro
- macro manager
  - internal keyboard exec macros
  - keystroke macros
    - send to host and keyboard
- trigger macros
- create and edit macros

# tape
- record output and replay
- show keystroke input as it comes in

# serve
- control routing to files for k-server web service
  - serve files to host from keyboard
- render markdown
# tape
- record output and replay

# backup
- backup manager
  - to git
  - to rclone




