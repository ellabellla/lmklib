# Result helper functions
def ok(ok):
    if ok is None:
        return
    return {"ok":ok, "err":None}

def err(err):
    if err is None:
        return
    return {"ok":None, "err":err}

# Interface
def load_data(name, data):
    """Initialise new driver from key server config data

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
        
