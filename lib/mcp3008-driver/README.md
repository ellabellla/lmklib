# MCP3008 Driver
A key-server driver module for driving a MCP3008 ic.

## Configuration
The driver expects a json object as it's configuration data. The object must have a "clock_pin", "mosi_pin", "miso_pin", "select_pin", and "channels" field. The pin fields setup the SPI pins the ic is connected to and must be integers. The "channels" field is a list of all analog pins on the ic to bind the driver to. The analog pins will be bound in the order they are listed in the "channels" field.

```json
{
    "clock_pin": 0,
    "mosi_pin": 1,
    "miso_pin": 2,
    "select_pin": 3,
    "channels": [
        0,
        1,
        2,
        3
    ]
}
```