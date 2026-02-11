#!/usr/bin/env python3
"""
Check if a Stellar account exists and show its balance.
"""

from stellar_sdk import Server

# Your vanity address
PUBLIC_KEY = "GCG3JBOHF5LUGBN3BI5RHD7WBHWEBHAMGNKHO5IYVA7MQ3IPOSMILUKE"

# Connect to Stellar public network
server = Server("https://horizon.stellar.org")

print(f"Checking account: {PUBLIC_KEY}\n")

try:
    account = server.accounts().account_id(PUBLIC_KEY).call()

    print("✅ Account is ACTIVE on the Stellar network!\n")

    # Show balances
    print("Balances:")
    for balance in account['balances']:
        if balance['asset_type'] == 'native':
            print(f"  XLM: {balance['balance']}")
        else:
            asset_code = balance.get('asset_code', 'Unknown')
            print(f"  {asset_code}: {balance['balance']}")

    print(f"\nSequence number: {account['sequence']}")
    print(f"\nView on Stellar Expert:")
    print(f"https://stellar.expert/explorer/public/account/{PUBLIC_KEY}")

except Exception as e:
    if "Resource Missing" in str(e) or "404" in str(e):
        print("❌ Account does NOT exist yet on the Stellar network.")
        print("\nTo activate it:")
        print(f"1. Send at least 1.5 XLM from VALR to: {PUBLIC_KEY}")
        print(f"2. Wait 1-5 minutes for the transaction to process")
        print(f"3. Run this script again to verify\n")
    else:
        print(f"Error: {e}")