/** Run the standard CLI in an isolated browser worker. */
export function run(args, files = {}, options = {}) {
  const started = performance.now();
  const { timeoutMs = 30000, signal, onStage, captureFiles = [] } = options;
  // Invalid timer values can silently become a 1 ms deadline in JavaScript.
  if (!Number.isInteger(timeoutMs) || timeoutMs < 1 || timeoutMs > 2147483647) {
    return Promise.reject(new RangeError("timeoutMs must be an integer from 1 to 2147483647."));
  }
  if (signal?.aborted) return Promise.reject(new DOMException("Run cancelled.", "AbortError"));

  return new Promise((resolve, reject) => {
    let worker, timer;
    let finished = false;
    // Every outcome terminates the worker and removes the timer and abort listener.
    const finish = (error, result) => {
      if (finished) return;
      finished = true;
      clearTimeout(timer);
      signal?.removeEventListener("abort", cancel);
      worker?.terminate();
      if (error) reject(error);
      else resolve({ ...result, elapsedMs: performance.now() - started });
    };
    const cancel = () => finish(new DOMException("Run cancelled.", "AbortError"));
    try {
      signal?.addEventListener("abort", cancel, { once: true });
      timer = setTimeout(() => finish(new DOMException(`Run exceeded ${timeoutMs} ms.`, "TimeoutError")), timeoutMs);
      worker = new Worker(new URL("./worker.mjs", import.meta.url), { type: "module" });
      worker.onmessage = ({ data }) => {
        if (finished) return;
        if (data.type === "stage") {
          try { onStage?.(data.stage); }
          catch (error) { finish(error); }
        } else if (data.type === "error") finish(new Error(data.error));
        else finish(null, data.result);
      };
      worker.onerror = error => finish(new Error(`Could not load or run worker.mjs. Check that the complete Wasm package is being served: ${error.message}`));
      worker.onmessageerror = () => finish(new Error("Could not read a message from the CLI worker."));
      worker.postMessage({ args, files, captureFiles });
    } catch (error) { finish(error); }
  });
}
