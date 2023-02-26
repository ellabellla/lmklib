# WS 1inch5 Driver
A key-server driver module for driving a [Waveshare 1.5 Inch OLED](https://www.waveshare.com/wiki/1.5inch_OLED_Module). The driver assumes the screen is mounted upside down.

## Controlling the Screen
Keystrokes can be sent to the screen by switching the current HID to the screen HID. Changing the current interface of the screen is done by sending HID commands:

|Command|Explanation|
|-------|-----------|
|"wake"|Turns the screen on/wakes up the sleep.|
|"home"|Navigates to the home screen.|
|"variables"|Navigates to the variables screen.|
|"term"|Navigates to the terminal screen.|
|"exit"|Turns off the screen and navigates to home.|

## Interfaces

### Home
Shows the current time. Keys can be looked up by entering coord of the key. First type the x, then enter, then type the y, then enter to look it up. Pressing escape will move the curser back to x from y. Use the up and down arrows to select which layer to lookup the key on.

### Variables
Shows a list of all variables in pages navigable using the arrow keys. Press the number corresponding to a variable to view/edit it.

### Terminal
Provides a simple interface for running commands. Type a command then press enter to run it. Only commands that don't require input may be run.