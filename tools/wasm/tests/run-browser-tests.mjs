import { spawn } from "node:child_process";
import { once } from "node:events";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";

const root = fileURLToPath(new URL("../../../", import.meta.url));
const server = spawn("python3", ["-u", "-m", "http.server", "8767", "--bind", "127.0.0.1"], {
  cwd: root,
  stdio: ["ignore", "pipe", "inherit"],
});
let browser;
try {
  await new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error("HTTP server did not start within 10 seconds.")), 10000);
    server.once("error", error => { clearTimeout(timer); reject(error); });
    server.once("exit", code => { clearTimeout(timer); reject(new Error(`HTTP server exited with code ${code}.`)); });
    server.stdout.on("data", data => {
      if (data.toString().includes("Serving HTTP")) { clearTimeout(timer); resolve(); }
    });
  });
  browser = await chromium.launch();
  const page = await browser.newPage();
  page.on("console", message => console.log(`browser: ${message.text()}`));
  page.on("pageerror", error => console.error(error));
  await page.goto("http://127.0.0.1:8767/tools/wasm/tests/browser.html");
  try {
    await page.waitForFunction(() => /^(PASS|FAIL):/.test(document.title), null, { timeout: 180000 });
  } finally {
    console.log(await page.locator("#output").innerText());
  }
  if (!(await page.title()).startsWith("PASS:")) throw new Error("Wasm browser tests failed.");
} finally {
  await browser?.close();
  if (server.exitCode === null) {
    const exited = once(server, "exit");
    server.kill();
    await exited;
  }
}
