"""
Stellar DEX Swap Retry Script

Hammers the network every second with a path_payment_strict_send swap attempt.
Exits the moment one succeeds.
"""

import sys
import time

from dotenv import load_dotenv
import os

from stellar_sdk import (
    Keypair,
    Network,
    Server,
    TransactionBuilder,
    Asset,
)
from stellar_sdk.exceptions import BadRequestError

load_dotenv()

SECRET_KEY = os.getenv("SECRET_KEY")
SELL_ASSET_CODE = os.getenv("SELL_ASSET_CODE")
SELL_ASSET_ISSUER = os.getenv("SELL_ASSET_ISSUER")
BUY_ASSET_CODE = os.getenv("BUY_ASSET_CODE")
BUY_ASSET_ISSUER = os.getenv("BUY_ASSET_ISSUER", "")
AMOUNT = os.getenv("AMOUNT")
MIN_AMOUNT = os.getenv("MIN_AMOUNT", "0.0000001")

HORIZON_URL = "https://horizon.stellar.org"
BASE_FEE = 100_000  # 0.01 XLM — high fee for priority inclusion


def build_asset(code, issuer):
    """Return a native Asset for XLM or a credit Asset otherwise."""
    if code.upper() == "XLM" and not issuer:
        return Asset.native()
    return Asset(code, issuer)


def main():
    if not SECRET_KEY or SECRET_KEY.startswith("S..."):
        print("ERROR: Set a valid SECRET_KEY in .env")
        sys.exit(1)

    keypair = Keypair.from_secret(SECRET_KEY)
    public_key = keypair.public_key
    server = Server(horizon_url=HORIZON_URL)

    sell_asset = build_asset(SELL_ASSET_CODE, SELL_ASSET_ISSUER)
    buy_asset = build_asset(BUY_ASSET_CODE, BUY_ASSET_ISSUER)

    print(f"Wallet:  {public_key}")
    print(f"Selling: {AMOUNT} {SELL_ASSET_CODE}")
    print(f"Buying:  {BUY_ASSET_CODE} (min {MIN_AMOUNT})")
    print(f"Fee:     {BASE_FEE} stroops")
    print("-" * 60)

    attempt = 0
    while True:
        attempt += 1
        try:
            # Fresh account load each attempt for current sequence number
            account = server.load_account(public_key)

            tx = (
                TransactionBuilder(
                    source_account=account,
                    network_passphrase=Network.PUBLIC_NETWORK_PASSPHRASE,
                    base_fee=BASE_FEE,
                )
                .append_path_payment_strict_send_op(
                    destination=public_key,
                    send_asset=sell_asset,
                    send_amount=AMOUNT,
                    dest_asset=buy_asset,
                    dest_min=MIN_AMOUNT,
                    path=[],  # let the network find the best DEX route
                )
                .set_timeout(30)
                .build()
            )

            tx.sign(keypair)
            response = server.submit_transaction(tx)
            tx_hash = response["hash"]

            print(f"\n[#{attempt}] SUCCESS!")
            print(f"TX Hash: {tx_hash}")
            print(f"https://stellar.expert/explorer/public/tx/{tx_hash}")
            sys.exit(0)

        except BadRequestError as e:
            # Extract Stellar result codes from Horizon error response
            extras = getattr(e, "extras", None) or {}
            result_codes = extras.get("result_codes", {})
            op_codes = result_codes.get("operations", [])
            tx_code = result_codes.get("transaction", "")
            print(f"[#{attempt}] FAILED - tx: {tx_code}, ops: {op_codes}")

        except Exception as e:
            print(f"[#{attempt}] ERROR - {type(e).__name__}: {e}")

        time.sleep(1)


if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        print("\nStopped by user.")
        sys.exit(0)
