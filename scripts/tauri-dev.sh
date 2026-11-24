#!/bin/bash
# Tauri dev wrapper to ensure cargo is in PATH
set -e

# Load cargo environment
source "$HOME/.cargo/env" 2>/dev/null || true

# Run tauri dev
exec yarn tauri dev
