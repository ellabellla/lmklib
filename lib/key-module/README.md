# Key-module

A library for creating modules for Key-server in rust and python. A module can provide either external functions or drivers to the key server.

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
- interface: The interface type (Function or Driver)
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
def load_data(name, data):
    """Initialize new driver from key server config data

    Args:
        name (str): Name of the driver type
        data (str): Driver data

    Returns:
        Result:
            Ok (int): Id of initialized driver
            Err (str): String describing error
    """
    pass

def name(driver_id):
    """Fetch the name of the driver with the specified id

    Args:
        driver_id (int): Driver id

    Returns:
        Result: 
            Ok (str): Name of the driver
            Err (str):  String describing error
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