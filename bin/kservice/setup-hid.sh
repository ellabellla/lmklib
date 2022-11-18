#!/bin/bash

# Create gadget
mkdir /sys/kernel/config/usb_gadget/lmk
cd /sys/kernel/config/usb_gadget/lmk

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

echo "Ella Pash" > strings/0x409/manufacturer
echo "LMK" > strings/0x409/product
echo "0123456789" > strings/0x409/serialnumber

# Create HID function
mkdir functions/hid.keyboard

echo 1 > functions/hid.keyboard/protocol
echo 33 > functions/hid.keyboard/report_length # 8-byte reports
echo 1 > functions/hid.keyboard/subclass

# Write report descriptor
cp /usr/hid/keyboard.desc functions/hid.keyboard/report_desc

# Create MOUSE HID function
mkdir functions/hid.mouse

echo 0 > functions/hid.mouse/protocol
echo 0 > functions/hid.mouse/subclass
echo 5 > functions/hid.mouse/report_length

cp /usr/hid/mouse.desc functions/hid.mouse/report_desc

# Create MASS STORAGE function
mkdir functions/mass_storage.usb0
echo 1 > functions/mass_storage.usb0/stall
for (( c=0; c<8; c++ ))
do 
    if (( $c != 0 ))
    then
        mkdir "functions/mass_storage.usb0/lun.$c"
    fi
    echo 0 > "functions/mass_storage.usb0/lun.$c/cdrom"
    echo 0 > "functions/mass_storage.usb0/lun.$c/ro"
    echo 0 > "functions/mass_storage.usb0/lun.$c/nofua"
    echo 1 > "functions/mass_storage.usb0/lun.$c/removable"
    chmod 777 "functions/mass_storage.usb0/lun.$c/file"
done

# Create Ethernet Adapter
mkdir -p functions/ecm.usb0
# first byte of address must be even
HOST="de:ca:ff:c0:ff:ee"
SELF="ca:55:1e:c0:ff:ee"
echo $HOST > functions/ecm.usb0/host_addr
echo $SELF > functions/ecm.usb0/dev_addr

# Create configuration
mkdir configs/c.1
mkdir configs/c.1/strings/0x409

echo 250 > configs/c.1/MaxPower # 200 mA
echo "Configuration" > configs/c.1/strings/0x409/configuration

# Link HID function to configuration
ln -s functions/hid.keyboard configs/c.1/
ln -s functions/hid.mouse configs/c.1/
ln -s functions/mass_storage.usb0 configs/c.1/
ln -s functions/ecm.usb0 configs/c.1/

# Enable gadget
ls /sys/class/udc > UDC

chmod 777 /dev/hidg0
chmod 777 /dev/hidg1

ifconfig usb0 10.0.0.1 netmask 255.255.255.252 up
route add -net default gw 10.0.0.2