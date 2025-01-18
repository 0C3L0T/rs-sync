#!/bin/bash

CLIENT_COMMAND="cargo run -- --verbose local-input/ 127.0.0.1:/home/gejsi/Desktop/project-laks-vjerdha/remote-output"
SERVER_COMMAND="cargo run -- --verbose --server=\"\""

start_client() {
  eval $CLIENT_COMMAND
}

start_server() {
  eval $SERVER_COMMAND
}

case "$1" in
  client)
    start_client
    ;;
  server)
    start_server
    ;;
  *)
    echo "Usage: $0 {client|server}"
    exit 1
    ;;
esac
