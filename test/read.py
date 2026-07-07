import socket
import threading
import time
import statistics
from queue import Queue

HOST = "127.0.0.1"
PORT = 4000

THREADS = 500          # concurrent clients
REQUESTS_PER_THREAD = 500

latencies = []
errors = 0
lock = threading.Lock()


def worker():
    global errors

    try:
        sock = socket.create_connection((HOST, PORT))
        sock.settimeout(5)

        # Test LIST TABLES command
        sock.sendall(b"LIST TABLES\n")
        response = sock.recv(4096).decode()
        print(f"LIST TABLES response: {response.strip()}")

        # Test DESCRIBE command for each table
        for table in ["products", "product", "productinfo", "productinfos", "pitem"]:
            sock.sendall(f"DESCRIBE {table}\n".encode())
            response = sock.recv(4096).decode()
            print(f"DESCRIBE {table} response: {response.strip()}")

        for _ in range(REQUESTS_PER_THREAD):
            start = time.perf_counter()

            # Test multiple tables
            for table in ["products", "product", "productinfo"]:
                sock.sendall(f"SELECT * FROM {table}\n".encode())
                sock.recv(4096)

            elapsed = (time.perf_counter() - start) * 1000

            with lock:
                latencies.append(elapsed)

        sock.close()

    except Exception as e:
        with lock:
            errors += 1
        print(f"Error: {e}")


def benchmark():
    start = time.perf_counter()

    threads = []

    for _ in range(THREADS):
        t = threading.Thread(target=worker)
        t.start()
        threads.append(t)

    for t in threads:
        t.join()

    total = time.perf_counter() - start

    total_requests = THREADS * REQUESTS_PER_THREAD

    print("=" * 40)
    print("Benchmark Results")
    print("=" * 40)
    print(f"Total Requests : {total_requests}")
    print(f"Concurrency    : {THREADS}")
    print(f"Errors         : {errors}")
    print(f"Total Time     : {total:.2f} sec")
    print(f"QPS            : {total_requests/total:.2f}")

    if latencies:
        print(f"Avg Latency    : {statistics.mean(latencies):.2f} ms")
        print(f"Median         : {statistics.median(latencies):.2f} ms")

        l = sorted(latencies)

        def percentile(p):
            idx = int(len(l) * p / 100)
            return l[min(idx, len(l)-1)]

        print(f"P95            : {percentile(95):.2f} ms")
        print(f"P99            : {percentile(99):.2f} ms")
        print(f"Max            : {max(latencies):.2f} ms")


if __name__ == "__main__":
    benchmark()