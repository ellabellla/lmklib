FROM ghcr.io/cross-rs/arm-unknown-linux-gnueabihf:latest

RUN curl https://www.alsa-project.org/files/pub/lib/alsa-lib-1.2.8.tar.bz2 > alsa.tar.bz2 && \
    tar -xf alsa.tar.bz2 && \
    cd alsa-lib-1.2.8/ && \
    ./configure --host=arm-unknown-linux-gnueabihf && \
    make install