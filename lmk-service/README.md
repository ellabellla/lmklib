# Raspberry Pi HID Setup Service

## Usage
Run "build-reports.sh" to rebuild the report descriptor binaries from xml. Uses [hidrd-convert](https://github.com/DIGImend/hidrd) to build the binaries.

Run "install.sh", with sudo, to install the service using systemd. Copies the files "hid.service" into "/etc/systemd/system/", "setup-hid.sh" into "/usr/bin/", and the report descriptors into "/usr/hid/".