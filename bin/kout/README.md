# Kout
A command line program to convert piped in text or files to key strokes and send them over the HID interface.

## Example
```bash
echo 'Hello, world!' | kout
```
## Usage
```
Usage: kout [INPUTS]...

Arguments:
  [INPUTS]...  Optional input files ('-' can be passed to mean stdio)

Options:
  -h, --help     Print help information
  -V, --version  Print version information
```