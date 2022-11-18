# Keyboard Layouts FORK

A fork of Chris Ricketts's Keyboard Layouts library. The fork removes all binaries and strips down the library to it's core functions.

Get the keycodes and modifier keys required to type an ASCII string for a number of different keyboard layouts. 

Takes inspiration and the [initial layout mappings](https://github.com/PaulStoffregen/cores/blob/master/teensy3/keylayouts.h) from the [Teensyduino project](https://github.com/PaulStoffregen/cores).

It works by preprocessing a C header file that describes the key mappings for each layout, including any deadkeys using `#define`'s. It then uses [bindgen](https://docs.rs/bindgen/0.47.2/bindgen/) to convert those into Rust constants and then [syn](https://docs.rs/syn/0.15.26/syn/) to extract the relevant keycodes and masks. It finally uses [quote!](https://docs.rs/quote/0.6.11/quote/) and [lazystatic!](https://docs.rs/lazy_static/1.2.0/lazy_static/) to produce a layout map enabling you to switch keyboard layouts on the fly without recompilation. 

## Supported Layouts 
 - Spanish
 - Canadian French
 - German Mac
 - German Swiss
 - Icelandic
 - United Kingdom
 - Italian
 - French Swiss
 - Finnish
 - Danish
 - French
 - German
 - Turkish
 - French Belgian
 - Portuguese
 - Canadian Multilingual
 - Spanish Latin America
 - US English
 - US International
 - Swedish
 - Portuguese Brazilian
 - Irish
Norwegian

## Testing 

Testing all the layouts are correct is hard. As a result the tests are hacky.

Testing for each layout is split into alphanumeric and symbols.
Each test:
1. Sets the user session's keyboard layout (only in plain virtual console, no X)
1. Creates a virtual HID device on the machine using /dev/uhid (user needs permissions)
1. Writes all the specified characters to the virtual HID device (cursor needs to be in the testing terminal and stay there)
1. Reads the string of types from stdin and compares with the original.

