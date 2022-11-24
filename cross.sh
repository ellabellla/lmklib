cross build --bin $2 --release --target=arm-unknown-linux-gnueabihf
scp target/arm-unknown-linux-gnueabihf/release/$2 $1:/home/ella/lmklib-rel/$2