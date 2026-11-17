#!/usr/bin/env bash
# A command that prints to both streams and fails, for the action smoke test of `wrap`.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

echo "starting"
echo "broken on purpose" >&2
exit 7
