import json

drivers = []

def load_data(name, data):
    if name == "Const":
        try:
            data = json.loads(data)
        except Exception as e:
            return "Invalid data"
        
        if "name" not in  data or "state" not in data:
            return "Missing fields in data"
        if not isinstance(data["name"], str) or not isinstance(data["state"], list):
            return "Wrong data field types"
        for state in data["state"]:
            if not isinstance(state, int):
                return "State must contain only ints"
        drivers.append(data)
        return len(drivers) - 1
    else:
        return "Unknown driver"


def name(driver_id):
    if driver_id < len(drivers):
        return {"ok":drivers[driver_id]["name"], "err": None}
    else:
        return {"err":"Unknown id", "ok": None}

def poll(driver_id):
    if driver_id < len(drivers):
        return drivers[driver_id]["state"]
    else:
        return "Unknown id"
        
