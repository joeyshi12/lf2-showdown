// A network game between the web build in headless Chromium and desktop programs
// (examples/net_peer) through the rendezvous server. The browser hosts (clicks
// "network game", "server", and presses Enter once everyone is in) or joins (types
// the room code a net_peer host printed). The other computers are net_peer
// processes; the first desktop computer goes through the menus into a VS match.
// The state hashes every computer reports must be the same.
// With PRESS=1 the browser's player taps attack every 450 ms during the game.
// usage: node tools/web/net_check.mjs URL host|join [COMPUTERS] [FRAMES]
// (URL e.g. http://127.0.0.1:8080/; needs chromium, LF2_DIR and a built net_peer)
import { spawn } from "child_process";
import readline from "readline";
const [url, role, nArg, fArg] = process.argv.slice(2);
const N = +nArg || 2;
const FRAMES = +fArg || 900;
const port = 9334;
const chrome = spawn("chromium", ["--headless=new", "--no-sandbox", "--disable-gpu", `--remote-debugging-port=${port}`,
  "--window-size=900,700", "about:blank"], { stdio: "ignore" });
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
let targets;
for (let i = 0; i < 50 && !targets; i++) {
  try { targets = await (await fetch(`http://127.0.0.1:${port}/json`)).json(); } catch { await sleep(200); }
}
const ws = new WebSocket(targets.find((t) => t.type === "page").webSocketDebuggerUrl);
await new Promise((r) => (ws.onopen = r));
let id = 0;
const pending = new Map(), errors = [];
ws.onmessage = (m) => {
  const d = JSON.parse(m.data);
  if (d.id && pending.has(d.id)) { pending.get(d.id)(d); pending.delete(d.id); }
  if (d.method === "Runtime.exceptionThrown") errors.push(d.params.exceptionDetails.exception?.description || d.params.exceptionDetails.text);
  if (d.method === "Runtime.consoleAPICalled" && process.env.CONSOLE) console.log("console:", d.params.args.map((a) => a.value).join(" "));
  if (d.method === "Page.javascriptDialogOpening") { errors.push("dialog: " + d.params.message); send("Page.handleJavaScriptDialog", { accept: true }); }
};
const send = (method, params = {}) => new Promise((r) => { const i = ++id; pending.set(i, r); ws.send(JSON.stringify({ id: i, method, params })); });
const evalJs = async (e) => (await send("Runtime.evaluate", { expression: e, returnByValue: true, awaitPromise: true })).result?.result?.value;
await send("Runtime.enable");
await send("Page.enable");
await send("Page.navigate", { url });
for (let i = 0; i < 300; i++) {
  if (await evalJs(`document.getElementById("screen").style.display === "block"`)) break;
  await sleep(200);
}
await sleep(1500);
const pos = await evalJs(`(() => { const b = document.getElementById("screen").getBoundingClientRect(); return [b.left, b.top]; })()`);
const click = async (x, y) => {
  for (const type of ["mouseMoved", "mousePressed", "mouseReleased"]) {
    await send("Input.dispatchMouseEvent", { type, x: pos[0] + x, y: pos[1] + y, button: "left", clickCount: 1 });
    await sleep(type === "mouseMoved" ? 300 : 150);
  }
  await sleep(500);
};
const key = async (code, k, vk, hold = 120) => {
  await send("Input.dispatchKeyEvent", { type: "keyDown", code, key: k, windowsVirtualKeyCode: vk });
  await sleep(hold);
  await send("Input.dispatchKeyEvent", { type: "keyUp", code, key: k, windowsVirtualKeyCode: vk });
  await sleep(200);
};
const peers = [];
const lines = [];
const startPeer = (args, env) => {
  const k = peers.length;
  const p = spawn(process.cwd() + "/target/release/examples/net_peer", args, { env: { ...process.env, FRAMES: String(FRAMES), ...env } });
  p.stderr.on("data", (d) => process.stderr.write(`peer ${k}: ` + d));
  lines[k] = [];
  const st = { proc: p, code: null, done: null };
  readline.createInterface({ input: p.stdout }).on("line", (l) => {
    if (l.startsWith("code ")) st.code = l.slice(5);
    else if (l.startsWith("hash ")) lines[k].push(l.slice(5));
    else if (l.startsWith("done")) st.done = l;
  });
  peers.push(st);
  return st;
};
await click(0x150, 0x100);                 // network game
if (role === "host") {
  await click(397, 243);                   // Host a game
  let code;
  for (let i = 0; i < 50 && !code; i++) { code = await evalJs(`window.lf2Net.code`); await sleep(200); }
  console.log("browser hosts room", code);
  for (let k = 1; k < N; k++) { startPeer(["join", code], { DRIVER: "1" }); await sleep(1500); }
  for (let i = 0; i < 150 && (await evalJs(`window.lf2Game().net_players()`)) < N; i++) await sleep(200);
  await key("Enter", "Enter", 0x0d);
} else {
  const host = startPeer(["host", String(N)], {});
  for (let i = 0; i < 100 && !host.code; i++) await sleep(100);
  console.log("desktop hosts room", host.code);
  for (let k = 2; k < N; k++) { startPeer(["join", host.code], {}); await sleep(1500); }
  await click(397, 277);                   // Join a game
  for (const c of host.code) await key(/[0-9]/.test(c) ? "Digit" + c : "Key" + c, c.toLowerCase(), c.charCodeAt(0));
  await key("Enter", "Enter", 0x0d);
}
const t0 = Date.now();
let taps = 0;
while (peers.some((p) => !p.done) && Date.now() - t0 < 100000) {
  if (process.env.PRESS && lines[0].length > 3) { await key("KeyJ", "j", 0x4a, 150); taps++; await sleep(100); }
  else await sleep(500);
}
await sleep(500);
const browser = ((await evalJs(`window.lf2Game().net_log()`)) || "").split("\n").filter((l) => l);
let ok = true;
for (let k = 0; k < peers.length; k++) {
  const d = lines[k];
  let same = 0, first = -1;
  for (let i = 0; i < Math.min(d.length, browser.length); i++) {
    if (d[i] === browser[i]) same++; else if (first < 0) first = i;
  }
  console.log(`desktop ${k}: ${d.length} hashes, browser ${browser.length}; identical ${same}` +
    (first >= 0 ? `; first difference at ${first}: ${d[first]} | ${browser[first]}` : "") + `; ${peers[k].done || "not done"}`);
  if (first >= 0 || same < d.length - 2 || d.length < FRAMES / 16 - 2) ok = false;
}
console.log("browser: frames, rollbacks, frames run again, waits:", await evalJs(`window.lf2Game().net_stats()`));
if (process.env.PRESS) console.log(`browser taps of attack: ${taps}`);
console.log(errors.length ? "page errors:\n" + errors.join("\n") : "no page errors");
for (const p of peers) p.proc.kill();
ws.close();
chrome.kill();
process.exit(ok && !errors.length ? 0 : 1);
