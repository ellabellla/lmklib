#!/bin/bash

mkdir /usr/gadget/
cp keyboard.desc /usr/gadget/keyboard.desc
cp mouse.desc /usr/gadget/mouse.desc
cp gadget-schema.json /usr/gadget/gadget-schema.json

cp gadget.service /etc/systemd/system/gadget.service

systemctl daemon-reload
systemctl enable gadget.service
