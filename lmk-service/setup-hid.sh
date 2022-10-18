#!/bin/sh

# Create gadget
mkdir /sys/kernel/config/usb_gadget/mykeyboard
cd /sys/kernel/config/usb_gadget/mykeyboard

# Add basic information
echo 0x0100 > bcdDevice # Version 1.0.0
echo 0x0200 > bcdUSB # USB 2.0
echo 0x00 > bDeviceClass
echo 0x00 > bDeviceProtocol
echo 0x00 > bDeviceSubClass
echo 0x08 > bMaxPacketSize0
echo 0x0104 > idProduct # Multifunction Composite Gadget
echo 0x1d6b > idVendor # Linux Foundation

# Create English locale
mkdir strings/0x409

echo "My manufacturer" > strings/0x409/manufacturer
echo "My virtual keyboard" > strings/0x409/product
echo "0123456789" > strings/0x409/serialnumber

# Create HID function
mkdir functions/hid.keyboard

echo 1 > functions/hid.keyboard/protocol
echo 16 > functions/hid.keyboard/report_length # 8-byte reports
echo 1 > functions/hid.keyboard/subclass

# Write report descriptor
cp /usr/hid/keyboard.desc functions/hid.keyboard/report_desc

# Create MOUSE HID function
mkdir functions/hid.mouse

echo 0 > functions/hid.mouse/protocol
echo 0 > functions/hid.mouse/subclass
echo 5 > functions/hid.mouse/report_length

cp /usr/hid/mouse.desc functions/hid.mouse/report_desc

# Create configuration
mkdir configs/c.1
mkdir configs/c.1/strings/0x409

echo 200 > configs/c.1/MaxPower # 200 mA
echo "Example configuration" > configs/c.1/strings/0x409/configuration

# Link HID function to configuration
ln -s functions/hid.keyboard configs/c.1/
ln -s functions/hid.mouse configs/c.1/

# Enable gadget
ls /sys/class/udc > UDC

chmod 777 /dev/hidg0
chmod 777 /dev/hidg1
