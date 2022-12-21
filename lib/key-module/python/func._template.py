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
    """Initialise new function from key server config data

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