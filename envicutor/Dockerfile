FROM rust:1.79-bookworm AS build
WORKDIR /app
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    cargo build --release && \
    mkdir /release && \
    cp ./target/release/envicutor ./target/release/delete_submission /release

FROM buildpack-deps@sha256:d56cd472000631b8faca51f40d4e3f1b20deffa588f9f207fa6c60efb62ba7c4 AS isolate
RUN apt-get update && \
    apt-get install -y --no-install-recommends git libcap-dev && \
    rm -rf /var/lib/apt/lists/* && \
    git clone https://github.com/envicutor/isolate.git /tmp/isolate/ && \
    cd /tmp/isolate && \
    git checkout af6db68042c3aa0ded80787fbb78bc0846ea2114 && \
    make -j$(nproc) install && \
    rm -rf /tmp/*

FROM debian:bookworm-20240612
RUN useradd -m envicutor && \
    mkdir -p /envicutor/runtimes && \
    chown envicutor:envicutor /envicutor/runtimes && \
    apt-get update && \
    apt-get install -y sqlite3 curl xz-utils && \
    install -d -m755 -o envicutor -g envicutor /nix
COPY --from=build /release/envicutor /release/delete_submission /envicutor/
COPY --from=isolate /usr/local/bin/isolate /usr/local/bin
COPY --from=isolate /usr/local/etc/isolate /usr/local/etc/isolate
COPY ./setup-cgroup.sh ./setup-nix-and-db.sh ./db.sql /envicutor
RUN chmod u+s /envicutor/delete_submission
CMD ["/envicutor/setup-cgroup.sh"]
