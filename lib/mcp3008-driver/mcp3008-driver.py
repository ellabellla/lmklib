from gpiozero import MCP3008
import json

drivers = []

def load_data(data):
    try:
        data = json.loads(data)
    except Exception as e:
        return "Invalid data"
    
    if "clock_pin" not in data \
        or "mosi_pin" not in data \
        or "miso_pin" not in data \
        or "select_pin" not in data \
        or "channels" not in data:
        return "Missing fields in data"
    
    channels = []
    for channel in data["channels"]:
        channels.append(
            MCP3008(
                channel=channel, 
                clock_pin=data["clock_pin"], 
                mosi_pin=data["mosi_pin"], 
                miso_pin=data["miso_pin"], 
                select_pin=data["select_pin"]
            )
        )

    drivers.append(channels)
    return len(drivers) - 1

def poll(driver_id):
    if driver_id < len(drivers):
        return [channel.raw_value for channel in drivers[driver_id]]
    else:
        return "Unknown id"
    
def set(driver_id, idx, state):
    pass
    
        
