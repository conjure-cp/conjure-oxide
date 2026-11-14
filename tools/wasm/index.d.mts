export interface RunOptions {
  /** Wall-clock limit including startup; defaults to 30000 ms. */
  timeoutMs?: number;
  signal?: AbortSignal;
  onStage?: (stage: "loading" | "running") => void;
  /** Files to return as UTF-8 text after the CLI finishes; missing files reject. */
  captureFiles?: string[];
}
export interface RunResult {
  exitCode: number;
  stdout: string;
  stderr: string;
  elapsedMs: number;
  files: Record<string, string>;
}
/** Run CLI arguments with input files in a private in-memory filesystem.
 * Nonzero CLI exit codes resolve normally if requested files can be read.
 * Worker or filesystem failures, cancellation, and timeout reject the Promise. */
export function run(args: string[], files?: Record<string, string>, options?: RunOptions): Promise<RunResult>;
