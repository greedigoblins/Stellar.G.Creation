#!/usr/bin/env python3
"""
Derive the public key from a secret key.
"""

from stellar_sdk import Keypair

secret_key = input("Enter secret key: ").strip()

try:
    kp = Keypair.from_secret(secret_key)
    print(f"\nPublic key: {kp.public_key}")
except Exception as e:
    print(f"Error: {e}")
    exit(1)
