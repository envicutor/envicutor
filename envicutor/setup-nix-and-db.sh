#!/bin/bash

id

if [ ! -d "/nix/store" ]; then
    echo /nix/store was not found, installing nix && \
    sh <(curl -L https://nixos.org/nix/install) --no-daemon || exit 1
    echo Installed nix successfully
else
    echo /nix/store was found
fi

sqlite3 /envicutor/runtimes/runtimes.db < /envicutor/db.sql && \
echo "Initialized the database" && \
cd /tmp # So PWD does not get leaked in env vars
exec /envicutor/envicutor
