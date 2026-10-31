import sys
import threading
import time

N = 300_000


def fib(n):
    a, b = 0, 1
    for _ in range(n):
        a, b = b, a + b
    return a


def bench_serial(jobs):
    start = time.perf_counter()
    for _ in range(jobs):
        fib(N)
    return time.perf_counter() - start


def bench_threaded(jobs):
    threads = [threading.Thread(target=fib, args=(N,)) for _ in range(jobs)]
    start = time.perf_counter()
    for t in threads:
        t.start()
    for t in threads:
        t.join()
    return time.perf_counter() - start


if __name__ == "__main__":
    mode = sys.argv[1]
    if mode == "one":
        start = time.perf_counter()
        fib(N)
        print(f"{time.perf_counter() - start:.3f}")
    elif mode == "serial4":
        print(f"{bench_serial(4):.3f}")
    elif mode == "threaded4":
        print(f"{bench_threaded(4):.3f}")
