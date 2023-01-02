import json

drivers = []

def load_data(data):
    try:
        data = json.loads(data)
    except Exception as e:
        return "Invalid data"
    
    if "state" not in data:
        return "Missing fields in data"
    if not isinstance(data["state"], list):
        return "Wrong data field types"
    for state in data["state"]:
        if not isinstance(state, int):
            return "State must contain only ints"
    drivers.append(data)
    return len(drivers) - 1

def poll(driver_id):
    if driver_id < len(drivers):
        return drivers[driver_id]["state"]
    else:
        return "Unknown id"
    
def set(driver_id, idx, state):
    if driver_id < len(drivers):
        if idx >= len(drivers[driver_id]["state"]):
            return "Idx out of bounds"
        drivers[driver_id]["state"][idx] = state
    else:
        return "Invalid id"
    
        
