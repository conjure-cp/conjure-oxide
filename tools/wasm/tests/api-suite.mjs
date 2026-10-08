function assert(condition, message) {
  if (!condition) throw new Error(message);
}
function equal(actual, expected) {
  assert(JSON.stringify(actual) === JSON.stringify(expected), `${JSON.stringify(actual)} != ${JSON.stringify(expected)}`);
}
const booleans = "find x, y : bool\nsuch that x != y\n";
const parameterised = "language Essence 1.3\ngiven n : int(1..5)\nwhere n > 1\nfind x : int(1..n)\nsuch that x = n\n";

export async function runCaptureSuite(run, log = () => {}) {
  const source = "find x : int(1..3)\nsuch that x = 3\n";
  for (const solver of ["minion", "sat"]) {
    const result = await run(
      ["solve", "/model.essence", "--solver", solver, "--output-format", "json", "-o", "/solutions.json"],
      { "/model.essence": source, "/empty.txt": "", "/notes.txt": "café\nλ\n" },
      { captureFiles: ["/solutions.json", "/model.essence", "/empty.txt", "/notes.txt"] },
    );
    assert(result.exitCode === 0, JSON.stringify(result));
    equal(JSON.parse(result.files["/solutions.json"]), [{ x: 3 }]);
    equal(result.files["/model.essence"], source);
    equal(result.files["/empty.txt"], "");
    equal(result.files["/notes.txt"], "café\nλ\n");
    equal(Object.keys(result.files), ["/solutions.json", "/model.essence", "/empty.txt", "/notes.txt"]);
    assert(result.stdout.includes("Solutions saved to"), "stdout is separate from file contents");
    const unsatisfiable = await run(
      ["solve", "/model.essence", "--solver", solver, "--output-format", "json", "-o", "/solutions.json"],
      { "/model.essence": "find x : int(1..3)\nsuch that x > 3\n" },
      { captureFiles: ["/solutions.json"] },
    );
    assert(unsatisfiable.exitCode === 0, JSON.stringify(unsatisfiable));
    equal(JSON.parse(unsatisfiable.files["/solutions.json"]), []);
  }
  equal((await run(["--help"])).files, {});
  for (const args of [["--help"], ["unknown-command"]]) {
    let failure;
    try { await run(args, {}, { captureFiles: ["/missing.json"] }); }
    catch (error) { failure = error; }
    assert(failure?.message.includes("read output file /missing.json after CLI exit code"), "missing file must fail explicitly");
    if (args[0] === "unknown-command") {
      assert(failure.message.includes("unrecognized subcommand"), "capture failure must retain CLI diagnostics");
    }
  }
  const concurrent = await Promise.all([3, 4].map(n => run(
    ["solve", "/model.essence", "/model.param", "--output-format", "json", "-o", "/solutions.json"],
    { "/model.essence": parameterised, "/model.param": `letting n be ${n}\n` },
    { captureFiles: ["/solutions.json"] },
  )));
  equal(concurrent.map(result => {
    assert(result.exitCode === 0, JSON.stringify(result));
    return JSON.parse(result.files["/solutions.json"]);
  }), [[{ x: 3 }], [{ x: 4 }]]);
  log("PASS: UTF-8 file capture, both solvers, multiple/empty files, missing-file errors, and concurrent files");
}

// Test the standard CLI through the browser worker, using ordinary arguments and output.
export async function runSuite(run, log = () => {}) {
  const solve = async (source, solver = "minion", limit = 20, params = "") => {
    const args = ["solve", "/model.essence"];
    const files = { "/model.essence": source };
    if (params) { args.push("/model.param"); files["/model.param"] = params; }
    args.push("--solver", solver, "-n", String(limit), "--output-format", "json");
    return run(args, files);
  };
  const ok = async (...inputs) => {
    const result = await solve(...inputs);
    assert(result.exitCode === 0, JSON.stringify(result));
    assert(Number.isFinite(result.elapsedMs) && result.elapsedMs >= 0, "elapsed time missing");
    return JSON.parse(result.stdout);
  };
  for (const solver of ["minion", "sat"]) {
    equal(await ok("find x : bool\nsuch that x\n", solver), [{ x: true }]);
    equal(await ok("find x : bool\nsuch that x, !x\n", solver), []);
    for (let i = 0; i < 2; i++) {
      const answer = await ok(booleans, solver);
      equal(answer.map(s => `${s.x},${s.y}`).sort(), ["false,true", "true,false"]);
      assert(answer.every(s => typeof s.x === "boolean" && typeof s.y === "boolean"), "JSON booleans");
    }
    equal(await ok("find x, y : int(1..5)\nsuch that x < y, x + y = 6\n", solver), [{ x: 1, y: 5 }, { x: 2, y: 4 }]);
    const matrix = await ok("find x : matrix indexed by [int(1..2)] of bool\nsuch that x[1] != x[2]\n", solver);
    equal(matrix.map(s => `${s.x["1"]},${s.x["2"]}`).sort(), ["false,true", "true,false"]);
    equal(await ok("find x : int(1..3)\nsuch that x > 3\n", solver), []);
    assert((await ok(booleans, solver, 1)).length === 1, "solution limit");
    const thousand = await ok("find x : int(1..1001)\n", solver, 1000);
    const values = thousand.map(solution => solution.x);
    assert(values.length === 1000 && new Set(values).size === 1000 && values.every(x => x >= 1 && x <= 1001), "1000 distinct valid solutions");
    equal((await ok(booleans, solver, 1000)).length, 2);
    equal(await ok("letting n be 3\nfind x : int(1..n)\nsuch that x = n\n", solver), [{ x: 3 }]);
    for (const n of [3, 4]) equal(await ok(parameterised, solver, 20, `letting n be ${n}\n`), [{ x: n }]);
    for (const params of ["letting n be", "letting m be 3", "letting n be 6", "letting n be 1", "letting n be true"]) {
      const result = await solve(parameterised, solver, 20, params);
      assert(result.exitCode !== 0 && result.stderr.length > 0, JSON.stringify(result));
    }
    for (const source of ["find x :", "find x : bool\nsuch that x + 1 = 2\n", "given n : int(1..5)\nfind x : int(1..n)\n"]) {
      const result = await solve(source, solver);
      assert(result.exitCode !== 0 && result.stderr.length > 0, JSON.stringify(result));
    }
    log(`PASS: ${solver}: satisfiable/unsatisfiable models, repeated runs, JSON values, parameters, 1000 solutions, and CLI errors`);
  }
  const help = await run(["--help"]);
  assert(help.exitCode === 0 && help.stdout.includes("solve"), "CLI help");
  const essence = await run(["solve", "/model.essence"], { "/model.essence": "find x : bool\nsuch that x\n" });
  assert(essence.exitCode === 0 && essence.stdout.includes("letting x be true"), "ordinary Essence output");
  for (const [args, context] of [
    [["solve", "/model.essence", "--solver", "z3"], "not compiled in"],
    [["server-lsp"], "not compiled in"],
    [["solve", "/model.essence", "--parser", "via-conjure"], "Could not find correct conjure executable"],
    [["unknown-command"], "unrecognized subcommand"],
  ]) {
    const result = await run(args, { "/model.essence": booleans });
    assert(result.exitCode !== 0 && result.stderr.includes(context), JSON.stringify(result));
  }
  for (const limit of [0, -1, Infinity, Number.MAX_SAFE_INTEGER + 1, 1.5]) {
    const result = await solve(booleans, "minion", limit);
    assert(result.exitCode !== 0 && result.stderr.length > 0, JSON.stringify(result));
  }
  const rejects = async (options, name) => {
    try { await run(["--help"], {}, options); }
    catch (error) { assert(error.name === name, String(error)); return; }
    throw new Error(`Expected ${name}`);
  };
  for (const timeoutMs of [0, Infinity]) await rejects({ timeoutMs }, "RangeError");
  const preCancelled = new AbortController(); preCancelled.abort();
  await rejects({ signal: preCancelled.signal }, "AbortError");
  const controller = new AbortController();
  await rejects({ signal: controller.signal, onStage: () => controller.abort() }, "AbortError");
  await rejects({ timeoutMs: 1 }, "TimeoutError");
  await rejects({ onStage: () => { throw new Error("callback failed"); } }, "Error");
  const stages = [];
  await run(["--help"], {}, { onStage: stage => stages.push(stage) });
  equal(stages, ["loading", "running"]);
  const concurrent = await Promise.all([
    ok(parameterised, "minion", 1, "letting n be 3\n"),
    ok("given n : int(1..5)\nfind y : int(1..n)\nsuch that y = n\n", "sat", 1, "letting n be 4\n"),
  ]);
  equal(concurrent, [[{ x: 3 }], [{ y: 4 }]]);
  log("PASS: raw arguments/stdout/stderr, unavailable features, cancellation, timeout, recovery, and isolated concurrent files");
}
