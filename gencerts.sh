#!/bin/bash

set -euxo pipefail

SCRIPT_DIR=$(dirname "$(readlink -f "$0")")

# openssl create self-signed certs
openssl req -x509 -newkey rsa:4096 -keyout ${SCRIPT_DIR}/key.pem -out ${SCRIPT_DIR}/cert.pem -days 365 -nodes -subj '/CN=localhost'
