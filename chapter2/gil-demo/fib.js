const { Worker, isMainThread, parentPort } = require("worker_threads");

const N = 300_000;

function fib(n) {
  let a = 0n, b = 1n;
  for (let i = 0; i < n; i++) {
    [a, b] = [b, a + b];
  }
  return a;
}

if (!isMainThread) {
  fib(N);
  parentPort.postMessage("done");
  return;
}

const mode = process.argv[2];

function benchSerial(jobs) {
  const start = performance.now();
  for (let i = 0; i < jobs; i++) fib(N);
  return (performance.now() - start) / 1000;
}

function benchThreaded(jobs) {
  return new Promise((resolve) => {
    const start = performance.now();
    let done = 0;
    for (let i = 0; i < jobs; i++) {
      const w = new Worker(__filename);
      w.on("message", () => {
        done++;
        if (done === jobs) resolve((performance.now() - start) / 1000);
      });
    }
  });
}

(async () => {
  if (mode === "one") {
    const start = performance.now();
    fib(N);
    console.log(((performance.now() - start) / 1000).toFixed(3));
  } else if (mode === "serial4") {
    console.log(benchSerial(4).toFixed(3));
  } else if (mode === "threaded4") {
    console.log((await benchThreaded(4)).toFixed(3));
  }
})();
