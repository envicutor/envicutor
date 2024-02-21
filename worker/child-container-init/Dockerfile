FROM rust:alpine3.19 AS build

# compile the init program
WORKDIR /app
RUN cargo init
COPY Cargo.toml Cargo.lock /app/
RUN apk update && apk add musl-dev && cargo build --release && rm -rf -- /app/*
COPY . .
RUN cargo build --release

FROM alpine:3.19.1

RUN apk update && apk add xz curl bash shadow && apk add --repository=http://dl-cdn.alpinelinux.org/alpine/edge/testing/ nsjail

# install nix
RUN /bin/bash -c "sh <(curl -L https://nixos.org/nix/install) --daemon --yes"\
    && useradd -m envicutor\
    && chmod u+s /usr/bin/nsjail || exit 1

USER envicutor

WORKDIR /app
COPY --from=build /app/target/release/child-container-init /app

CMD ["/app/child-container-init"]
