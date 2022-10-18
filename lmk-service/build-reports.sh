rm -f keyboard.desc
rm -f mouse.desc
hidrd-convert -i xml -o natv keyboard.xml > keyboard.desc
hidrd-convert -i xml -o natv mouse.xml > mouse.desc