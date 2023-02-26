# Key-RPC
A library for interfacing with a key-server over nanomsg RPC.


## Example
```rust
let client = Client::new("ipc:///lmk/ksf.ipc").unwrap();
println!("{}", client.layer().unwrap());
```