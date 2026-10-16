#!/usr/bin/env sh
# Generates the throwaway test CA and server certificate for the Dovecot
# integration test container. Run before `docker compose up --build`.
# The output is git-ignored, so no private key ever lands in the repository.
set -eu
mkdir -p "$(dirname "$0")/certs"
cd "$(dirname "$0")/certs"

openssl req -x509 -newkey rsa:2048 -nodes -days 36500 \
  -subj "/CN=managesieve test CA" \
  -addext "basicConstraints=critical,CA:TRUE" \
  -addext "keyUsage=critical,keyCertSign,cRLSign" \
  -keyout ca.key -out ca.crt

openssl req -newkey rsa:2048 -nodes \
  -subj "/CN=localhost" \
  -keyout tls.key -out tls.csr

cat > tls.ext <<EOF
basicConstraints=critical,CA:FALSE
keyUsage=critical,digitalSignature,keyEncipherment
extendedKeyUsage=serverAuth
subjectAltName=DNS:localhost,IP:127.0.0.1,IP:::1
EOF

openssl x509 -req -in tls.csr -CA ca.crt -CAkey ca.key -CAcreateserial \
  -days 36500 -extfile tls.ext -out tls.crt

rm -f tls.csr tls.ext ca.srl ca.key
chmod 644 tls.key
