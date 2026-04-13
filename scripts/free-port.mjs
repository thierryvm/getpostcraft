/**
 * Kills whatever process is listening on PORT before Vite starts.
 * Works on Windows (netstat + taskkill) and Unix (fuser).
 * PORT is a hardcoded constant — no user input, no injection risk.
 * Called by the "dev" npm script — no extra dependencies needed.
 */
import { execSync, execFileSync } from "node:child_process";

const PORT = 1420;

function freeWindows() {
  try {
    const out = execSync("netstat -ano", { encoding: "utf8" });
    const re = new RegExp(
      `(?:127\\.0\\.0\\.1|0\\.0\\.0\\.0|\\[::1\\]|\\[::0\\]):${PORT}\\s+\\S+\\s+LISTENING\\s+(\\d+)`,
      "gm"
    );
    const pids = new Set();
    let m;
    while ((m = re.exec(out)) !== null) pids.add(m[1]);
    for (const pid of pids) {
      try {
        // execFileSync avoids shell — args are passed directly to taskkill
        execFileSync("taskkill", ["/PID", pid, "/F"], { stdio: "ignore" });
        console.log(`[free-port] killed PID ${pid} on :${PORT}`);
      } catch {
        // process may have exited already
      }
    }
  } catch {
    // netstat unavailable — skip silently
  }
}

function freePosix() {
  try {
    // fuser -k sends SIGKILL to the owning process; ignore if port is free
    execFileSync("fuser", ["-k", `${PORT}/tcp`], { stdio: "ignore" });
  } catch {
    // nothing was listening
  }
}

if (process.platform === "win32") {
  freeWindows();
} else {
  freePosix();
}
