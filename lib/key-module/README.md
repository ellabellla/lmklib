# Key-module

A library for creating modules for Key-server in rust and python. A module can provide either a HID output, external functions or drivers to the key server.

Rust modules are written using abi_stable and are dynamically linked. Python modules are loaded and interpreted at runtime using Pyo3.

This crate contains the abi_stable interfaces for rust modules and some template files for python modules.

## Anatomy of a Module

Modules are folders containing:
- module.py(Python)/module.so(Rust): The module to be loaded
- meta.json: The meta data of the module

All modules are placed in the modules folder inside the key-server config folder.

## Meta.json
```json
{
  "name": "",
  "interface":"",
  "module_type":""
}
```

A json file containing an object with the items:
- name: The name of the module
- interface: The interface type (HID, Function or Driver)
- module_type: The module type (ABIStable or Python)

## Python Templates

### Result Return Helpers
```python
def ok(ok):
    if ok is None:
        return
    return {"ok":ok, "err":None}

def err(err):
    if err is None:
        return
    return {"ok":None, "err":err}
```
### Driver Module
```python
def load_data(data):
    """Initialize new driver from key server config data

    Args:
        data (str): Driver data

    Returns:
        Result:
            Ok (int): Id of initialized driver
            Err (str): String describing error
    """
    pass

def poll(driver_id):
    """Poll the current state of the driver with the specified id

    Args:
        driver_id (int): Driver id

    Returns:
        Result: 
            Ok (list): List of the states (int)
            Err (str):  String describing error
    """
    pass

def set(driver_id, idx, state):
    """Set the current state of the driver with the specified id

    Args:
        driver_id (int): Driver id
        idx: (int) State index
        state (int): The state

    Returns:
        Result: 
            Ok (None)
            Err (str): String describing error
    """
    pass

```

### Function Module
```python
def load_data(name, data):
    """Initialize new function from key server config data

    Args:
        name (str): Name of the function type
        data (str): Function data

    Returns:
        Result:
            Ok (int): Id of initialized function
            Err (str): String describing error
    """
    pass

def event(func_id, state):
    """Keyboard pool event, runs every time the keyboard polls the state associated with the function

    Args:
        func_id (int): Function id
        state (list): list of the states (int)

    Returns:
        Result: 
            Ok (None)
            Err (str): String describing error
    """
    pass
```

### HID Module
```python
def hold_key(key):
    """Hold key event

    Args:
        key (int): char as integer of held key

    Returns:
        None
    """
    pass

def hold_special(special):
    """Hold special key  event

    Args:
        special (int): special key id

    Returns:
        None
    """
    pass

def hold_modifier(modifier):
    """Hold modifier key  event

    Args:
        modifier (int): modifier key id

    Returns:
        None
    """
    pass

def release_key(key):
    """Release key event

    Args:
        key (int): char as integer of released key

    Returns:
        None
    """
    pass

def release_special(special):
    """Release special key  event

    Args:
        special (int): special key id

    Returns:
        None
    """
    pass

def release_modifier(modifier):
    """Release modifier key  event

    Args:
        modifier (int): modifier key id

    Returns:
        None
    """
    pass

def press_basic_str(str):
    """Type string event

    Args:
        str (str): string to type

    Returns:
        None
    """
    pass

def press_str(layout, str):
    """Type string based on layout event

    Args:
        layout (str): layout name
        str (str): string to type

    Returns:
        None
    """
    pass

def scroll_wheel(amount):
    """Scroll wheel by amount

    Args:
        amount (int): amount to move

    Returns:
        None
    """
    pass

def move_mouse_x(amount):
    """Move mouse in x direction by amount

    Args:
        amount (int): amount to move

    Returns:
        None
    """
    pass

def move_mouse_y(amount):
    """Move mouse in y direction by amount

    Args:
        amount (int): amount to move

    Returns:
        None
    """
    pass

def hold_button(button):
    """Hold mouse button event

    Args:
        button (int): mouse button id

    Returns:
        None
    """
    pass

def release_button(button):
    """Release mouse button event

    Args:
        button (int): mouse button id

    Returns:
        None
    """
    pass

def send_command(data):
    """Command sent event (used to send custom commands to a hid)

    Args:
        data (str): command data

    Returns:
        None
    """
    pass

def send_keyboard():
    """Send buffered keyboard data

    Args:

    Returns:
        None
    """
    pass

def send_mouse():
    """Send buffered mouse data

    Args:

    Returns:
        None
    """
    pass

```