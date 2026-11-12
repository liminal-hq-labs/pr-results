#!/bin/bash
# Regenerates the test certificate authorities and the leaf certificate for the loopback TLS tests.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

# These files are for tests only: two throwaway certificate authorities (`test-ca.pem` signs
# `leaf.pem`, `other-ca.pem` signs nothing), and the leaf's private key. Their private keys are not
# kept, so nothing can be signed with them again. The dates are fixed far apart so a test never
# depends on the day it runs. Needs OpenSSL 3.4 or later.
#
# Usage: generate.sh   (writes into this directory)
set -euo pipefail
cd "$(dirname "$0")"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
window=(-not_before 20200101000000Z -not_after 21201231000000Z)

for name in test other; do
	openssl ecparam -name prime256v1 -genkey -noout -out "$work/$name.key" 2>/dev/null
	openssl req -new -x509 -key "$work/$name.key" -sha256 "${window[@]}" \
		-subj "/CN=pr-results $name CA (loopback tests only)" \
		-addext "basicConstraints=critical,CA:TRUE" \
		-addext "keyUsage=critical,keyCertSign,cRLSign" -out "$name-ca.pem"
done

openssl ecparam -name prime256v1 -genkey -noout -out "$work/leaf.sec1.key" 2>/dev/null
openssl pkcs8 -topk8 -nocrypt -in "$work/leaf.sec1.key" -out leaf.key
openssl req -new -key "$work/leaf.sec1.key" -subj "/CN=localhost" -out "$work/leaf.csr"
printf '%s\n' \
	"subjectAltName=IP:127.0.0.1,DNS:localhost" \
	"basicConstraints=critical,CA:FALSE" \
	"keyUsage=critical,digitalSignature" \
	"extendedKeyUsage=serverAuth" >"$work/leaf.ext"
openssl x509 -req -in "$work/leaf.csr" -CA test-ca.pem -CAkey "$work/test.key" -CAcreateserial \
	-CAserial "$work/test.srl" -sha256 "${window[@]}" -extfile "$work/leaf.ext" -out leaf.pem 2>/dev/null
