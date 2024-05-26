FROM rust:alpine3.19 AS build
# compile the init program
RUN apk update && apk add musl-dev
WORKDIR /app
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
  --mount=type=cache,target=/app/target \
  cargo build --release && \
  mkdir /release && \
  cp -r ./target/release/envicutor /release/envicutor


FROM alpine:3.19.1
# install nix dependencies, set up nix unprivileged user, nix directory, setuid bit in nsjail binary
RUN apk update && apk add xz curl bash shadow \
  && apk add --repository=http://dl-cdn.alpinelinux.org/alpine/edge/testing/ nsjail \
  && useradd -m envicutor \
  && install -d -m755 -o envicutor -g envicutor /nix \
  && chmod u+s /usr/bin/nsjail
# use the unprivileged user
USER envicutor
# install nix
RUN /bin/bash -c "curl -L https://nixos.org/nix/install | sh"
# run the binary
WORKDIR /app
COPY --from=build /release/envicutor /app
CMD ["/app/envicutor"]
