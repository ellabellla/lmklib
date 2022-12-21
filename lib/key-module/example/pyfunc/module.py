func = []
def load_data(name, data):
    if name == "PyPrint":
        func.append((data, 0))
        return len(func) - 1
    else:
        return "Unknown function"

def event(func_id, state):
    if func_id < len(func):
        data, prev_state = func[func_id]
        if state != 0 and prev_state == 0:
            print("PyPrint: " + data)
        func[func_id] = (data, state)
    else:
        return "Invalid id"
