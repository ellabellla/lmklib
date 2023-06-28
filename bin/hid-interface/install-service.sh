mkdir -p $HOME/.config/systemd/user
cp hid.service $HOME/.config/systemd/user/hid.service
systemctl --user daemon-reload
systemctl --user enable hid.service
systemctl --user start hid.service
