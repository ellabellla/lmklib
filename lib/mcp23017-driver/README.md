# MCP23017 Driver
A key-server driver module for driving a MCP23017 ic.

## Configuration
The driver expects a json object as it's configuration data. The object must have an "address", and "inputs" field and optionally a "bus" field". The "bus" field specifies which i2c bus the ic's are connected too, but default it is 1. The "address" field is a list of all of the ic i2c addresses this driver should connect too. The ic's will be connected in the order specified. The "inputs" field is a list of all input bindings for the driver. The pin numbers setup for the ic's will start at 0 and then increment across the ic's connected. e.g. pin 0 on the second ic will be 16.

An input binding is an object containing one of three fields: "Matrix", "Input", and "Output". 

The "matrix" field contains an object with the fields "x", "y", and optionally "invert". It defines a matrix of pins used as an input. "x" and "y" defines the pins making up the axes of the matrix. The "x" pins will be used as outputs and the "y" pins will be used as inputs. When "invert" is set to true this will be reversed. 

The "input" field contains an object with the fields "pin", "on_state", and "pull_high". It defines a single pin as an input. "pin" defines which pin will act as an input. "on_state" defines the value of the pin that is considered to be high. e.g. true = high when pin is high, false = high when pin is low. "pull_high" when true will enable the ic's internal pull high resistor for that pin.

The "output" field contains an object with the fields "pin". It defines a single pin as an input. "pin" defines which pin will act as an output.

```json
{
    "address": [30],
    "bus": 3,
    "inputs": [
        {
            "input": {
                "pin": 0,
                "on_state": true,
                "pull_high": false,
            }
        }
    ]
}
```