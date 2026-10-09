// Open the web build in headless Chromium, drive it through the title screen into a
// match with keys and the mouse, and save screenshots of the canvas.
// usage: node tools/web/browser_check.mjs URL OUT_PREFIX
// (URL is e.g. http://127.0.0.1:8765/index.html?game=game; needs chromium on PATH)
import { spawn } from "child_process";
import fs from "fs";
const [url, out] = process.argv.slice(2);
const port = 9333;
const chrome = spawn("chromium", ["--headless=new", "--no-sandbox", "--disable-gpu", `--remote-debugging-port=${port}`,
  "--autoplay-policy=no-user-gesture-required", "--window-size=900,700", "about:blank"], { stdio: "ignore" });
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
let targets;
for (let i = 0; i < 50 && !targets; i++) {
  try { targets = await (await fetch(`http://127.0.0.1:${port}/json`)).json(); } catch { await sleep(200); }
}
const ws = new WebSocket(targets.find((t) => t.type === "page").webSocketDebuggerUrl);
await new Promise((r) => (ws.onopen = r));
let id = 0;
const pending = new Map();
const errors = [];
ws.onmessage = (m) => {
  const d = JSON.parse(m.data);
  if (d.id && pending.has(d.id)) { pending.get(d.id)(d); pending.delete(d.id); }
  if (d.method === "Runtime.exceptionThrown") errors.push(d.params.exceptionDetails.exception?.description || d.params.exceptionDetails.text);
  if (d.method === "Runtime.consoleAPICalled" && d.params.type === "error") errors.push(d.params.args.map((a) => a.value || a.description).join(" "));
};
const send = (method, params = {}) => new Promise((r) => { const i = ++id; pending.set(i, r); ws.send(JSON.stringify({ id: i, method, params })); });
const evalJs = async (e) => (await send("Runtime.evaluate", { expression: e, returnByValue: true })).result?.result?.value;
await send("Runtime.enable");
await send("Page.enable");
await send("Page.navigate", { url });
for (let i = 0; i < 300; i++) {
  if (await evalJs(`document.getElementById("screen").style.display === "block"`)) break;
  await sleep(200);
}
const shot = async (name) => {
  const r = await evalJs(`(() => { const b = document.getElementById("screen").getBoundingClientRect(); return [b.left, b.top]; })()`);
  const s = await send("Page.captureScreenshot", { format: "png", clip: { x: r[0], y: r[1], width: 794, height: 550, scale: 1 } });
  fs.writeFileSync(`${out}.${name}.png`, Buffer.from(s.result.data, "base64"));
};
const pos = await evalJs(`(() => { const b = document.getElementById("screen").getBoundingClientRect(); return [b.left, b.top]; })()`);
const mouse = async (type, x, y) => send("Input.dispatchMouseEvent", { type, x: pos[0] + x, y: pos[1] + y, button: "left", clickCount: 1 });
const key = async (code, k, vk) => {
  await send("Input.dispatchKeyEvent", { type: "keyDown", code, key: k, windowsVirtualKeyCode: vk });
  await sleep(150);
  await send("Input.dispatchKeyEvent", { type: "keyUp", code, key: k, windowsVirtualKeyCode: vk });
  await sleep(450);
};
await sleep(2000);
await shot("title");
if (process.env.CURSOR) {
  // move the mouse over the title screen for a second and count how many of the
  // pictures shown differ from the one before (the cursor is the only thing moving)
  await evalJs(`(() => { window.__pics = []; const c = document.getElementById("screen").getContext("2d");
    const put = c.putImageData.bind(c); c.putImageData = (img, x, y) => { put(img, x, y);
      let h = 0; const d = img.data; for (let i = 0; i < d.length; i += 61) h = (h * 31 + d[i]) | 0;
      window.__pics.push(h); }; })()`);
  const t0 = Date.now();
  for (let i = 0; Date.now() - t0 < 1000; i++) {
    await mouse("mouseMoved", 100 + (i % 300), 300);
    await sleep(4);
  }
  const pics = await evalJs(`window.__pics`);
  let changed = 0;
  for (let i = 1; i < pics.length; i++) if (pics[i] !== pics[i - 1]) changed++;
  console.log(`cursor: ${pics.length} pictures in 1 s, ${changed} differ from the one before`);
  await evalJs(`(() => { const c = document.getElementById("screen").getContext("2d"); delete c.putImageData; })()`);
}
await mouse("mouseMoved", 400, 230); await sleep(300);
await mouse("mousePressed", 400, 230); await sleep(150); await mouse("mouseReleased", 400, 230);
await sleep(2500);
await shot("menu");
const J = ["KeyJ", "j", 0x4a], W = ["KeyW", "w", 0x57];
await key(...J); await sleep(1500);
for (let i = 0; i < 4; i++) await key(...J);
await sleep(7000);
for (let i = 0; i < 4; i++) await key(...J);
await key(...W); await key(...W); await key(...J);
await sleep(4000);
await shot("match");
if (process.env.FRAMES) {
  // keep the next N shown pictures with their times; player 1 runs right meanwhile
  await evalJs(`(() => { window.__shots = []; const c = document.getElementById("screen").getContext("2d");
    const put = c.putImageData.bind(c); c.putImageData = (img, x, y) => { put(img, x, y);
    if (window.__shots.length < ${+process.env.FRAMES}) window.__shots.push([performance.now(), c.canvas.toDataURL()]); }; })()`);
  await send("Input.dispatchKeyEvent", { type: "keyDown", code: "KeyD", key: "d", windowsVirtualKeyCode: 0x44 });
  await sleep(150);
  await send("Input.dispatchKeyEvent", { type: "keyUp", code: "KeyD", key: "d", windowsVirtualKeyCode: 0x44 });
  await sleep(60);
  await send("Input.dispatchKeyEvent", { type: "keyDown", code: "KeyD", key: "d", windowsVirtualKeyCode: 0x44 });
  await sleep(1500);
  const shots = await evalJs(`window.__shots`);
  shots.forEach(([t, url], i) => fs.writeFileSync(`${out}.f${String(i).padStart(2, "0")}.png`, Buffer.from(url.split(",")[1], "base64")));
  console.log("times", shots.map(([t]) => Math.round(t - shots[0][0])).join(" "));
}
console.log(JSON.stringify(await evalJs(`window.lf2Status()`)));
console.log(errors.length ? "errors:\n" + errors.join("\n") : "no errors");
ws.close();
chrome.kill();
process.exit(0);
