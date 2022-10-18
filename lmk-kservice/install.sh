#!/bin/bash

mkdir /usr/hid/
cp keyboard.desc /usr/hid/keyboard.desc
cp mouse.desc /usr/hid/mouse.desc

cp setup-hid.sh /usr/bin/setup-hid.sh
cp hid.service /etc/systemd/system/hid.service

systemctl daemon-reload
systemctl enable hid.service
