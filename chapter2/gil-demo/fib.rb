N = 300_000

def fib(n)
  a, b = 0, 1
  n.times { a, b = b, a + b }
  a
end

def bench_serial(jobs)
  start = Process.clock_gettime(Process::CLOCK_MONOTONIC)
  jobs.times { fib(N) }
  Process.clock_gettime(Process::CLOCK_MONOTONIC) - start
end

def bench_threaded(jobs)
  start = Process.clock_gettime(Process::CLOCK_MONOTONIC)
  threads = Array.new(jobs) { Thread.new { fib(N) } }
  threads.each(&:join)
  Process.clock_gettime(Process::CLOCK_MONOTONIC) - start
end

case ARGV[0]
when "one"
  start = Process.clock_gettime(Process::CLOCK_MONOTONIC)
  fib(N)
  puts (Process.clock_gettime(Process::CLOCK_MONOTONIC) - start).round(3)
when "serial4"
  puts bench_serial(4).round(3)
when "threaded4"
  puts bench_threaded(4).round(3)
end
