#!/usr/bin/env python3
"""
Stellar Vanity Address Generator - CPU Multi-Core
Optimized for cloud instances with many CPU cores

Searches for: GOAT...LUKE
- Starts with GOAT (G + OAT prefix)
- Ends with LUKE suffix

Usage:
    python vanity_goat_luke.py
"""

import sys
import time
import multiprocessing as mp
from stellar_sdk import Keypair


PREFIX = "OAT"   # After the G, so address starts with GOAT
SUFFIX = "LUKE"


def worker(worker_id, result_queue, stop_event, stats_queue):
    """
    Worker process that generates keypairs and checks for GOAT...LUKE pattern.
    """
    attempts = 0
    start = time.time()
    last_report = start

    while not stop_event.is_set():
        # Generate random keypair
        kp = Keypair.random()
        public = kp.public_key
        attempts += 1

        # Check if matches pattern: GOAT...LUKE
        # public[0] is always 'G'
        # public[1:4] should be 'OAT' -> makes 'GOAT'
        # public should end with 'LUKE'
        if public[1:4] == PREFIX and public.endswith(SUFFIX):
            # FOUND IT!
            result_queue.put({
                'public': public,
                'secret': kp.secret,
                'attempts': attempts,
                'worker_id': worker_id,
                'time': time.time() - start
            })
            stop_event.set()
            return

        # Send stats every 5 seconds
        now = time.time()
        if now - last_report > 5:
            elapsed = now - start
            rate = attempts / elapsed if elapsed > 0 else 0
            stats_queue.put({
                'worker_id': worker_id,
                'attempts': attempts,
                'rate': rate,
                'elapsed': elapsed
            })
            last_report = now


def stats_monitor(num_workers, stats_queue, stop_event, start_time):
    """
    Monitor and display aggregate stats from all workers.
    """
    worker_stats = {}
    last_print = time.time()

    print("\n" + "="*70)
    print("Starting workers...\n")

    while not stop_event.is_set():
        # Collect stats from queue
        while not stats_queue.empty():
            try:
                stat = stats_queue.get_nowait()
                worker_stats[stat['worker_id']] = stat
            except:
                break

        # Print update every 5 seconds
        now = time.time()
        if now - last_print >= 5 and worker_stats:
            total_attempts = sum(s['attempts'] for s in worker_stats.values())
            total_rate = sum(s['rate'] for s in worker_stats.values())
            elapsed = now - start_time

            print(f"{'='*70}")
            print(f"Elapsed: {elapsed/60:.1f} min  |  Total attempts: {total_attempts:,}")
            print(f"Speed: {total_rate:,.0f} keys/sec  |  Active workers: {len(worker_stats)}/{num_workers}")

            # Show estimated time remaining
            expected_total = 34_359_738_368 / 2  # 32^7 / 2
            if total_rate > 0:
                remaining = (expected_total - total_attempts) / total_rate
                hours_remaining = remaining / 3600
                print(f"Estimated time remaining: {hours_remaining:.1f} hours")

            print(f"{'='*70}\n")
            last_print = now

        time.sleep(1)


def main():
    # Get number of CPU cores
    num_cores = mp.cpu_count()

    print("\n" + "="*70)
    print("Stellar Vanity Generator - GOAT...LUKE")
    print("="*70)
    print(f"\nPattern: G{PREFIX}...{SUFFIX}")
    print(f"Example: GOAT7XQZ4MHFWVBYN3K8JPRST6XAELUKE")

    # Calculate difficulty
    prefix_difficulty = 32 ** len(PREFIX)  # 32^3
    suffix_difficulty = 32 ** len(SUFFIX)  # 32^4
    total_difficulty = prefix_difficulty * suffix_difficulty  # 32^7

    print(f"\nDifficulty:")
    print(f"  Prefix 'OAT':  1 in {prefix_difficulty:,}")
    print(f"  Suffix 'LUKE': 1 in {suffix_difficulty:,}")
    print(f"  Combined:      1 in {total_difficulty:,} (average: {total_difficulty//2:,} attempts)")

    print(f"\nCPU Cores: {num_cores}")

    # Estimate performance
    keys_per_core = 7_000  # Conservative estimate
    estimated_rate = num_cores * keys_per_core
    expected_attempts = total_difficulty / 2
    estimated_hours = expected_attempts / estimated_rate / 3600

    print(f"Estimated speed: ~{estimated_rate:,} keys/sec")
    print(f"Estimated time: {estimated_hours:.1f} hours ({estimated_hours/24:.1f} days)")
    print("\n(You might get lucky and find it sooner!)")
    print("="*70)

    # Confirm before starting
    print("\nPress Enter to start, or Ctrl+C to cancel...")
    try:
        input()
    except KeyboardInterrupt:
        print("\nCancelled.")
        sys.exit(0)

    # Setup multiprocessing
    result_queue = mp.Queue()
    stats_queue = mp.Queue()
    stop_event = mp.Event()

    start_time = time.time()

    # Start stats monitor
    stats_proc = mp.Process(
        target=stats_monitor,
        args=(num_cores, stats_queue, stop_event, start_time)
    )
    stats_proc.start()

    # Start worker processes
    workers = []
    for i in range(num_cores):
        p = mp.Process(
            target=worker,
            args=(i, result_queue, stop_event, stats_queue)
        )
        p.start()
        workers.append(p)

    # Wait for result
    try:
        result = result_queue.get()  # Blocks until found
    except KeyboardInterrupt:
        print("\n\nStopped by user.")
        stop_event.set()
        stats_proc.terminate()
        for p in workers:
            p.terminate()
        sys.exit(1)

    # Stop everything
    stop_event.set()
    elapsed = time.time() - start_time

    stats_proc.join(timeout=1)
    if stats_proc.is_alive():
        stats_proc.terminate()

    for p in workers:
        p.join(timeout=1)
        if p.is_alive():
            p.terminate()

    # Display result
    print("\n" + "="*70)
    print("SUCCESS! Found matching address!")
    print("="*70)
    print(f"\nWorker #{result['worker_id']} found it after {result['attempts']:,} attempts")
    print(f"Worker time: {result['time']:.1f} seconds")
    print(f"Total time: {elapsed:.1f} seconds ({elapsed/60:.1f} minutes)")

    # Calculate total attempts across all workers (estimate)
    total_attempts_estimate = result['attempts'] * num_cores
    actual_rate = total_attempts_estimate / elapsed if elapsed > 0 else 0
    print(f"Estimated total attempts: ~{total_attempts_estimate:,}")
    print(f"Average speed: {actual_rate:,.0f} keys/sec")

    print(f"\n{'='*70}")
    print("YOUR NEW STELLAR KEYPAIR:")
    print(f"{'='*70}")
    print(f"\nPublic Key (G address):\n  {result['public']}")
    print(f"\nSecret Key (S secret - KEEP THIS SAFE!):\n  {result['secret']}")
    print(f"\n{'='*70}")

    # Save to file
    filename = "goat_luke_keypair.txt"
    with open(filename, "w") as f:
        f.write("="*70 + "\n")
        f.write("STELLAR VANITY KEYPAIR - GOAT...LUKE\n")
        f.write("="*70 + "\n\n")
        f.write(f"Public Key:\n{result['public']}\n\n")
        f.write(f"Secret Key:\n{result['secret']}\n\n")
        f.write("="*70 + "\n")
        f.write("IMPORTANT: Keep your secret key safe!\n")
        f.write("Anyone with this secret key can control this account.\n")
        f.write("="*70 + "\n\n")
        f.write(f"Found by worker #{result['worker_id']}\n")
        f.write(f"Worker attempts: {result['attempts']:,}\n")
        f.write(f"Estimated total attempts: ~{total_attempts_estimate:,}\n")
        f.write(f"Time: {elapsed:.1f} seconds ({elapsed/60:.1f} minutes)\n")
        f.write(f"Speed: {actual_rate:,.0f} keys/sec\n")

    print(f"\nKeypair saved to: {filename}")
    print("\n⚠️  IMPORTANT: Back up your secret key in a secure location!")
    print("="*70 + "\n")


if __name__ == "__main__":
    # Required for multiprocessing on Windows
    mp.freeze_support()
    main()
