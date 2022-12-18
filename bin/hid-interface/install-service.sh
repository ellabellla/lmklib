mkdir -p /home/ella/.config/systemd/user
cp hid.service /home/ella/.config/systemd/user/hid.service
systemctl --user daemon-reload
systemctl --user enable hid.service
systemctl --user start hid.service
