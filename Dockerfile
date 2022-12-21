FROM --platform=$BUILDPLATFORM balenalib/raspberrypi3-debian-python:3.9 as base

RUN apt-get update
RUN apt-get install --assume-yes libasound2 libasound2-dev libnanomsg-dev pkg-config
RUN apt-get install --assume-yes build-essential

COPY ./rustup-init.sh .
RUN ./rustup-init.sh -y --default-host arm-unknown-linux-gnueabihf

ENV PATH="$PATH:$HOME/.cargo/env"
ENV PKG_CONFIG_PATH=/usr/lib/arm-linux-gnueabihf/pkgconfig
ENV PYO3_NO_PYTHON=""

WORKDIR /app

VOLUME ["/root/.cargo/registry"]
VOLUME [ "/app" ]
VOLUME [ "/app/target" ]

RUN echo '\n[source.crates-io]\nreplace-with = "vendored-sources"\n[source.vendored-sources]\ndirectory = "vendor"' > /root/.cargo/Cargo.toml
