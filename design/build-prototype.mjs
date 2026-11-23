import { writeFileSync, readFileSync } from "node:fs";

const uiIcon = name => readFileSync(new URL('./vendor/lucide/'+name+'.svg', import.meta.url),'utf8').replace('<svg','<svg aria-hidden="true" focusable="false" class="ui-icon"');
const cards = [
  ["clay", "白模转写实", "将白模场景转成写实效果", `<rect class="ground" y="62" width="160" height="46"/><rect class="box" x="18" y="48" width="36" height="28"/><rect class="box tall" x="48" y="22" width="26" height="54"/><rect class="box" x="82" y="54" width="58" height="22"/>`],
  ["material", "材质替换", "修改物体材质与表面属性", `<rect class="ground" y="70" width="160" height="38"/><rect class="box" x="22" y="28" width="70" height="36" transform="rotate(-8 57 46)"/><rect class="box tall" x="68" y="36" width="64" height="28"/>`],
  ["character", "角色精修", "细节、质感与外观优化", `<rect class="ground" y="78" width="160" height="30"/><circle class="box tall" cx="58" cy="36" r="14"/><rect class="box" x="36" y="52" width="44" height="32" rx="8"/>`],
  ["mood", "场景氛围", "调整灯光、天气与环境气氛", `<rect class="ground" y="64" width="160" height="44"/><circle class="sun" cx="118" cy="28" r="10"/><rect class="box" x="24" y="58" width="28" height="22"/><rect class="box tall" x="58" y="40" width="18" height="40"/>`],
  ["expand", "图片扩展", "扩展画面边界与构图", `<rect class="box" x="36" y="22" width="88" height="64"/><rect class="ground" x="28" y="48" width="8" height="2"/><rect class="ground" x="124" y="48" width="8" height="2"/><rect class="ground" x="78" y="14" width="2" height="8"/><rect class="ground" x="78" y="86" width="2" height="8"/>`],
  ["style", "风格迁移", "转换视觉风格和艺术表现", `<rect class="ground" y="0" width="28" height="108" transform="skewX(-18)"/><rect class="box" x="36" y="0" width="22" height="108"/><rect class="box tall" x="78" y="0" width="16" height="108"/><rect class="ground" x="112" y="0" width="26" height="108"/>`],
];

const resultScene = `<svg class="scene result-scene" viewBox="0 0 300 360" preserveAspectRatio="xMidYMid slice" aria-hidden="true"><rect class="sky" width="300" height="360"/><rect class="ground" y="230" width="300" height="130"/><rect class="box" x="36" y="188" width="70" height="64"/><rect class="box tall" x="96" y="96" width="52" height="156"/><rect class="box" x="156" y="210" width="108" height="48"/><circle class="sun" cx="214" cy="64" r="22"/></svg>`;

function cardLink(theme, [id, title, desc, art]) {
  const href = theme === "dark" ? "#panel-dark-result" : "#panel-light-result";
  return `<a class="card" href="${href}" data-card="${title}" title="${desc}"><svg class="scene" viewBox="0 0 160 108" preserveAspectRatio="xMidYMid slice" aria-hidden="true"><rect class="sky" width="160" height="108"/>${art}</svg><span class="card-title">${title}</span></a>`;
}

function grid(theme) {
  return `<div class="grid">${cards.map((card) => cardLink(theme, card)).join("")}</div>`;
}

function header(closeHref, status) {
  const aside = status ? `<span class="status">${status}</span>` : "";
  return `<header class="bar"><div class="brand">${uiIcon("panel-top")}<span>QuickGen</span></div>${aside}<a class="close" href="${closeHref}" aria-label="关闭">${uiIcon("x")}</a></header>`;
}

function panel({ id, theme, state, closeHref }) {
  const label = `${theme === "dark" ? "Dark" : "Light"} / ${state === "result" ? "Result" : "Default"}`;
  const body = state === "result"
    ? `${header(closeHref, "白模转写实 · 已保存")}<div class="result-shot" draggable="true" title="拖动这张图" oncontextmenu="openMenu(event)">${resultScene}</div>`
    : `${header(closeHref, "")}${grid(theme)}`;
  return `<article class="frame" id="${id}"><p class="label">${label}</p><section class="panel ${theme}">${body}</section></article>`;
}

const panels = [
  panel({ id: "panel-dark-default", theme: "dark", state: "default", closeHref: "#flow-collapsed" }),
  panel({ id: "panel-light-default", theme: "light", state: "default", closeHref: "#flow-collapsed" }),
  panel({ id: "panel-dark-result", theme: "dark", state: "result", closeHref: "#flow-collapsed" }),
  panel({ id: "panel-light-result", theme: "light", state: "result", closeHref: "#flow-collapsed" }),
].join("\n");

function notch(href, label) {
  return `<a class="notch" href="${href}" aria-label="${label}">${uiIcon("panel-top")}</a>`;
}

function desktop(id, withPanel) {
  const shelf = withPanel
    ? `${notch("#flow-collapsed", "收回 QuickGen")}<section class="panel dark">${header("#flow-collapsed", "")}${grid("dark")}</section>`
    : notch("#flow-open", "拉出 QuickGen");
  const label = withPanel ? "Flow / 拉出" : "Flow / 刘海";
  return `<article class="frame" id="${id}"><p class="label">${label}</p><div class="desktop"><div class="dcc">视口</div><div class="shelf${withPanel ? " open" : ""}">${shelf}</div></div></article>`;
}

const sample = cards[0];
const swatches = ["bg", "surface", "border", "text", "muted"].map((token) => `<div class="swatch"><i data-token="${token}"></i><code>${token}</code></div>`).join("");

const html = `<!DOCTYPE html>
<html lang="zh-CN">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>QuickGen 交互原型</title>
<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
<link href="https://fonts.googleapis.com/css2?family=IBM+Plex+Sans:wght@400;500&display=swap" rel="stylesheet">
<style>
  :root { --radius-panel: 6px; --radius-card: 3px; --canvas: #101010; --label: #8a8a8a; color-scheme: dark; }
  * { box-sizing: border-box; }
  html { scroll-behavior: smooth; }
  body { margin: 0; background: var(--canvas); color: #f2f2f2; font-family: "IBM Plex Sans", "Segoe UI", sans-serif; font-size: 12px; line-height: 1.3; }
  a { color: inherit; text-decoration: none; }
  button { font: inherit; color: inherit; background: transparent; border: 0; }
  :focus-visible { outline: 1px solid currentColor; outline-offset: 2px; }
  main { padding: 28px 28px 64px; display: flex; flex-direction: column; gap: 36px; }
  h1 { font-size: 18px; font-weight: 500; letter-spacing: 0; margin: 0 0 6px; }
  .lede { max-width: 560px; color: #bdbdbd; margin: 0; }
  .start { display: inline-flex; margin-top: 12px; height: 28px; padding: 0 10px; align-items: center; border-radius: 3px; background: #f2f2f2; color: #141414; }
  h2 { font-size: 11px; font-weight: 500; letter-spacing: 0.08em; text-transform: uppercase; color: var(--label); margin: 0 0 10px; }
  .row { display: flex; gap: 20px; align-items: flex-start; flex-wrap: wrap; }
  .label { margin: 0 0 6px; color: var(--label); font-size: 11px; }
  .dark, .swatches.dark { --bg: #141414; --surface: #1c1c1c; --border: #2a2a2a; --text: #f2f2f2; --muted: #8a8a8a; color: var(--text); }
  .light, .swatches.light { --bg: #f4f4f2; --surface: #ffffff; --border: #d8d8d6; --text: #161616; --muted: #6b6b6b; color: var(--text); }
  .panel { width: 308px; padding: 6px; display: flex; flex-direction: column; gap: 4px; background: var(--bg); border: 1px solid var(--border); border-radius: var(--radius-panel); }
  .bar { height: 20px; display: flex; align-items: center; gap: 8px; padding: 0 2px; }
  .brand { display: flex; align-items: center; gap: 6px; font-size: 11px; font-weight: 500; letter-spacing: 0.01em; }
  .status { margin-left: auto; color: var(--muted); font-size: 10px; letter-spacing: 0.01em; }
  .close { margin-left: auto; width: 16px; height: 16px; display: grid; place-items: center; color: var(--muted); font-size: 14px; line-height: 1; }
  .status + .close { margin-left: 8px; }
  .mark { width: 9px; height: 9px; border: 1px solid currentColor; border-radius: 1px; display: inline-block; position: relative; }
  .mark::after { content: ""; position: absolute; inset: 2px; background: currentColor; }
  .grid { display: grid; grid-template-columns: 1fr 1fr; gap: 4px; }
  .card { position: relative; height: 96px; border-radius: var(--radius-card); overflow: hidden; border: 1px solid var(--border); display: block; }
  .card .scene { position: absolute; inset: 0; width: 100%; height: 100%; display: block; }
  .scene .sky { fill: color-mix(in srgb, var(--text) 8%, var(--bg)); }
  .scene .ground { fill: color-mix(in srgb, var(--text) 18%, var(--bg)); }
  .scene .box { fill: color-mix(in srgb, var(--text) 36%, var(--bg)); }
  .scene .box.tall, .scene .sun { fill: color-mix(in srgb, var(--text) 55%, var(--bg)); }
  .scene.result-scene .sky { fill: color-mix(in srgb, var(--text) 22%, var(--bg)); }
  .scene.result-scene .sun { fill: color-mix(in srgb, var(--text) 82%, var(--bg)); }
  .card-title { position: absolute; left: 8px; right: 8px; bottom: 7px; color: #f4f4f4; font-size: 11px; font-weight: 500; letter-spacing: 0.01em; text-shadow: 0 1px 2px rgba(0,0,0,.9); pointer-events: none; }
  .result-shot { height: 292px; border-radius: var(--radius-card); overflow: hidden; border: 1px solid var(--border); cursor: grab; }
  .result-shot:active { cursor: grabbing; }
  .result-shot .scene { width: 100%; height: 100%; display: block; }
  .desktop { width: 760px; height: 520px; position: relative; border-radius: 6px; background: #171717; border: 1px solid #2a2a2a; overflow: hidden; }
  .dcc { position: absolute; inset: 28px 16px 16px; border: 1px solid #2a2a2a; border-radius: 4px; color: #6a6a6a; padding: 8px; font-size: 11px; }
  .shelf { position: absolute; top: 0; left: 50%; transform: translateX(-50%); display: flex; flex-direction: column; align-items: center; z-index: 2; }
  .notch { width: 88px; height: 18px; border-radius: 0 0 9px 9px; background: #141414; border: 1px solid #2c2c2c; border-top: 0; color: #f2f2f2; display: flex; align-items: center; justify-content: center; z-index: 2; }
  .notch.hot, .notch:hover { background: #1c1c1c; }
  .shelf.open .notch { margin-bottom: -9px; }
  .edge-sample { position: relative; width: 240px; height: 56px; background: #171717; border: 1px solid #2a2a2a; border-radius: 4px; }
  .swatches { display: flex; gap: 8px; padding: 8px; border-radius: 4px; border: 1px solid var(--border); background: var(--bg); }
  .swatch { width: 56px; display: flex; flex-direction: column; gap: 4px; font-size: 10px; color: var(--muted); }
  .swatch i { height: 22px; border-radius: 2px; border: 1px solid var(--border); display: block; }
  .swatch i[data-token="bg"] { background: var(--bg); }
  .swatch i[data-token="surface"] { background: var(--surface); }
  .swatch i[data-token="border"] { background: var(--border); }
  .swatch i[data-token="text"] { background: var(--text); }
  .swatch i[data-token="muted"] { background: var(--muted); }
  .notes p { margin: 0 0 6px; color: #c8c8c8; max-width: 640px; }
  .menu { position: fixed; z-index: 5; display: none; min-width: 88px; padding: 2px; background: #1c1c1c; color: #f2f2f2; border: 1px solid #2e2e2e; border-radius: 3px; }
  .menu button { display: block; width: 100%; height: 24px; text-align: left; padding: 0 8px; border-radius: 2px; cursor: pointer; }
  .menu button:hover { background: #2a2a2a; }
  .toast { position: fixed; left: 50%; bottom: 16px; transform: translateX(-50%); background: #f2f2f2; color: #141414; border-radius: 3px; padding: 6px 10px; font-size: 11px; display: none; }
  .toast.show { display: block; }
  body.presenting { background: #0c0c0c; }
  body.presenting main > section, body.presenting .intro { display: none; }
  body.presenting main > section:has(:target) { display: block; }
  body.presenting .frame { display: none; }
  body.presenting .frame:target { display: block; margin: 28px auto; }
</style>
<link rel="stylesheet" href="prototype-extension.css?v=motion-preview-11">
</head>
<body>
<main id="board">
  <section class="intro">
    <h1>QuickGen</h1>
    <p class="lede">入口可拖动并贴靠顶部或左右侧边。点击它，或把图片拖到它上面，面板会从边缘拉出。图片铺满卡片，拖到卡片上就生成。</p>
    <a class="start" href="#flow-collapsed">从屏幕边缘开始</a>
  </section>
  <section class="foundations" id="foundations">
    <h2>Foundations</h2>
    <div class="row" style="margin-bottom:12px">
      <div><p class="label">Dark</p><div class="swatches dark">${swatches}</div></div>
      <div><p class="label">Light</p><div class="swatches light">${swatches}</div></div>
    </div>
    <p class="label">刘海贴边 · 卡片整面是图片，标题直接写在画面上</p>
    <div class="row">
      <div class="edge-sample">${notch("#flow-open", "拉出 QuickGen")}</div>
      <div class="dark" style="width:148px">${cardLink("dark", sample)}</div>
      <div class="light" style="width:148px">${cardLink("light", sample)}</div>
    </div>
  </section>
  <section id="panels">
    <h2>Panels</h2>
    <div class="row">${panels}</div>
  </section>
  <section id="flow">
    <h2>Flow</h2>
    <div class="row">
      ${desktop("flow-collapsed", false)}
      ${desktop("flow-open", true)}
    </div>
  </section>
  <section class="notes" id="notes">
    <h2>Notes</h2>
    <p>点击刘海，或把文件拖到刘海上，面板从所在边缘向内展开。再点一次刘海，面板收回。</p>
    <p>原型里点击卡片等于把图片拖到这张卡片上，随即进入结果。</p>
    <p>结果图可以拖动。在结果图上右键可以选择复制。生成完成后默认已经保存到本地。</p>
  </section>
</main>
<div class="menu" id="menu" role="menu"><button type="button" id="copy">复制</button></div>
<div class="toast" id="toast" role="status"></div>
<script>
  const menu = document.getElementById("menu");
  const toast = document.getElementById("toast");
  window.openMenu = function (event) {
    event.preventDefault();
    menu.style.display = "block";
    menu.style.left = event.clientX + "px";
    menu.style.top = event.clientY + "px";
  };
  function showNote(text) {
    toast.textContent = text;
    toast.classList.add("show");
    clearTimeout(showNote._t);
    showNote._t = setTimeout(() => toast.classList.remove("show"), 1600);
  }
  function present() {
    const frames = new Set(["#flow-collapsed", "#flow-open", "#panel-dark-default", "#panel-light-default", "#panel-dark-result", "#panel-light-result"]);
    document.body.classList.toggle("presenting", frames.has(location.hash));
  }
  window.addEventListener("hashchange", present);
  present();
  document.addEventListener("contextmenu", (event) => {
    if (!event.target.closest(".result-shot")) {
      menu.style.display = "none";
      return;
    }
    event.preventDefault();
    menu.style.display = "block";
    menu.style.left = event.clientX + "px";
    menu.style.top = event.clientY + "px";
  });
  document.addEventListener("click", (event) => {
    if (event.target.id === "copy") { showNote("已复制"); menu.style.display = "none"; }
    if (!event.target.closest("#menu")) menu.style.display = "none";
  });
  document.addEventListener("dragstart", (event) => {
    if (!event.target.closest(".result-shot")) return;
    event.dataTransfer.effectAllowed = "copy";
    event.dataTransfer.setData("text/plain", "quickgen-result.png");
  });
  document.querySelectorAll(".notch").forEach((node) => {
    node.addEventListener("dragover", (event) => {
      event.preventDefault();
      node.classList.add("hot");
    });
    node.addEventListener("dragleave", () => node.classList.remove("hot"));
    node.addEventListener("drop", (event) => {
      event.preventDefault();
      node.classList.remove("hot");
      location.hash = "#flow-open";
    });
  });
</script>
<script src="icons.js"></script>
<script src="prototype-extension.js?v=motion-preview-11"></script>
</body>
</html>`;

writeFileSync(new URL("./quickgen-prototype.html", import.meta.url), html);
console.log("wrote", html.length);
