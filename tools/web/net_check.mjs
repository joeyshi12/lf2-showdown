// A network game between the web build in headless Chromium and the desktop
// program (examples/net_peer) through the rendezvous server. The browser hosts
// (clicks "network game", "server") or joins (types the code net_peer printed);
// then the digests of the objects after each packet exchange, from both sides,
// must be the same.
// usage: node tools/web/net_check.mjs URL host|join EXCHANGES
// (URL e.g. http://127.0.0.1:8080/; needs chromium, LF2_DIR and a built net_peer)
import { spawn } from "child_process";
import readline from "readline";
const [url, role, nArg] = process.argv.slice(2);
const N = +nArg || 200;
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
const key = async (code, k, vk) => {
  await send("Input.dispatchKeyEvent", { type: "keyDown", code, key: k, windowsVirtualKeyCode: vk });
  await sleep(120);
  await send("Input.dispatchKeyEvent", { type: "keyUp", code, key: k, windowsVirtualKeyCode: vk });
  await sleep(200);
};
await click(0x150, 0x100);                 // network game
const peerArgs = [];
if (role === "host") {
  await click(0x150, 0x118);               // server
  let code;
  for (let i = 0; i < 50 && !code; i++) { code = await evalJs(`window.lf2Net.code`); await sleep(200); }
  console.log("browser hosts room", code);
  peerArgs.push("join", code);
} else {
  peerArgs.push("host");
}
const peer = spawn(process.cwd() + "/target/release/examples/net_peer", peerArgs, { env: { ...process.env, EXCHANGES: String(N) } });
peer.stderr.on("data", (d) => process.stderr.write("peer: " + d));
const peerLines = [];
const rl = readline.createInterface({ input: peer.stdout });
let peerCode = null;
rl.on("line", (l) => { if (l.startsWith("code ")) peerCode = l.slice(5); else if (l.includes("digest")) peerLines.push(l); });
if (role === "join") {
  for (let i = 0; i < 100 && !peerCode; i++) await sleep(100);
  console.log("desktop hosts room", peerCode);
  await click(0x150, 0x138);               // client
  for (const c of peerCode) await key(/[0-9]/.test(c) ? "Digit" + c : "Key" + c, c.toLowerCase(), c.charCodeAt(0));
  await key("Enter", "Enter", 0x0d);
}
const t0 = Date.now();
while (peerLines.length < N && Date.now() - t0 < 90000) await sleep(500);
await sleep(1000);
const browserLines = (await evalJs(`window.lf2Game().net_log()`) || "").split("\n").filter((l) => l);
let same = 0, firstDiff = -1;
for (let i = 0; i < Math.min(browserLines.length, peerLines.length); i++) {
  if (browserLines[i] === peerLines[i]) same++; else if (firstDiff < 0) firstDiff = i;
}
console.log(`exchanges: browser ${browserLines.length}, desktop ${peerLines.length}; identical ${same}` +
  (firstDiff >= 0 ? `; first difference at ${firstDiff}: ${browserLines[firstDiff]} | ${peerLines[firstDiff]}` : ""));
console.log("last:", peerLines[peerLines.length - 1]);
console.log(errors.length ? "page errors:\n" + errors.join("\n") : "no page errors");
peer.kill();
ws.close();
chrome.kill();
process.exit(firstDiff < 0 && same >= N - 1 ? 0 : 1);
