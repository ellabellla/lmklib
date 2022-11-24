# Raspberry Pi Gadget Setup Service

Controls a systemd service that configures usb gadget functionality required by LMK Lib. It can be used to install, uninstall, enable and disable the service. It can also be used to configure the usb gadget without the service running.

## Usage
```bash
Usage: gadget-service <COMMAND>

Commands:
  install    Install the gadget service
  uninstall  Uninstall the gadget service
  enable     Enable
  disable    Disable
  configure  Configure the usb gadget
  clean      Remove all files created during install
  help       Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help information
  -V, --version  Print version information
```

## Report Descriptors
Run "build-reports.sh" to rebuild the report descriptor binaries from xml. [hidrd-convert](https://github.com/DIGImend/hidrd) is required.
