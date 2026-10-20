// The TextCAD viewer: source, feature tree and 3D view of one part, linked both ways.
// It talks to `tcad serve` (see crates/tcad-cli/src/serve.rs and spec/viewer.md), or, on a page with
// `<body data-mode="browser">`, regenerates in the browser with the WebAssembly module (worker.ts,
// docs/adr/0014): the source becomes an editor and every edit rebuilds the part.

import * as THREE from "three";
import { GLTFLoader } from "three/examples/jsm/loaders/GLTFLoader.js";
import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";

interface Feature {
  name: string;
  keyword: string;
  faces: number;
  volume_mm3: number;
}

interface PartInfo {
  ok: boolean;
  source: string;
  configs: string[];
  build?: { features: Feature[]; result: { volume_mm3: number; faces: number } };
  error?: string;
  diagnostics?: unknown;
}

interface FaceMesh {
  mesh: THREE.Mesh;
  id: string;
  aliases: string[];
}

const KEYWORDS = new Set([
  "sketch", "extrude", "revolve", "sweep", "loft", "hole", "fillet", "chamfer", "shell", "draft",
  "rib", "linear_pattern", "circular_pattern", "mirror", "combine", "plane", "axis", "point", "csys",
]);

const el = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;

const scene = new THREE.Scene();
scene.background = new THREE.Color(0x20242b);
const camera = new THREE.PerspectiveCamera(40, 1, 0.1, 1e5);
camera.up.set(0, 0, 1);
const renderer = new THREE.WebGLRenderer({ antialias: true, canvas: el<HTMLCanvasElement>("view") });
const controls = new OrbitControls(camera, renderer.domElement);
scene.add(new THREE.HemisphereLight(0xffffff, 0x404858, 1.6));
const sun = new THREE.DirectionalLight(0xffffff, 1.4);
sun.position.set(1, -2, 3);
camera.add(sun);
scene.add(camera);

let faces: FaceMesh[] = [];
let group: THREE.Group | null = null;
let current = { file: "", config: "", version: "" };
let selectedFeature: string | null = null;
let picked: FaceMesh | null = null;
let features: Feature[] = [];
let lineOf = new Map<string, number>();

const browserMode = document.body.dataset.mode === "browser";

const BASE = new THREE.Color(0x9fb4cc);
const FEATURE = new THREE.Color(0xf2b134);
const PICKED = new THREE.Color(0xff5470);

function featureOf(id: string): string {
  return id.split(".")[0] ?? id;
}

function paint(): void {
  for (const f of faces) {
    const m = f.mesh.material as THREE.MeshStandardMaterial;
    const mine = selectedFeature !== null && featureOf(f.id) === selectedFeature;
    m.color.copy(f === picked ? PICKED : mine ? FEATURE : BASE);
  }
  document.querySelectorAll<HTMLElement>("#tree li").forEach((li) => {
    li.classList.toggle("sel", li.dataset.name === selectedFeature);
  });
  document.querySelectorAll<HTMLElement>("#source .line").forEach((div) => {
    const line = Number(div.dataset.line);
    div.classList.toggle("sel", selectedFeature !== null && lineOf.get(selectedFeature) === line);
  });
  el("status").textContent = picked
    ? `${picked.id}${picked.aliases.length ? "  (also " + picked.aliases.join(", ") + ")" : ""}`
    : selectedFeature
      ? `feature ${selectedFeature}`
      : "";
}

function selectFeature(name: string | null, reveal = true): void {
  selectedFeature = name;
  picked = null;
  paint();
  if (browserMode) {
    showStatementInEditor(name);
    return;
  }
  if (reveal && name !== null) {
    const line = lineOf.get(name);
    if (line !== undefined) {
      document.querySelector(`#source .line[data-line="${line}"]`)?.scrollIntoView({ block: "nearest" });
    }
  }
}

/// Line (1-based) of each feature's statement.
function scanLines(source: string): Map<string, number> {
  const lines = new Map<string, number>();
  source.split("\n").forEach((text, i) => {
    const m = /^\s*([a-z_]+)\s+([a-z_][a-z0-9_]*)\b/.exec(text);
    if (m && KEYWORDS.has(m[1]) && !lines.has(m[2])) lines.set(m[2], i + 1);
  });
  return lines;
}

function renderSource(source: string): void {
  lineOf = new Map();
  const box = el("source");
  box.textContent = "";
  source.split("\n").forEach((text, i) => {
    const div = document.createElement("div");
    div.className = "line";
    div.dataset.line = String(i + 1);
    const m = /^\s*([a-z_]+)\s+([a-z_][a-z0-9_]*)\b/.exec(text);
    if (m && KEYWORDS.has(m[1]) && !lineOf.has(m[2])) {
      lineOf.set(m[2], i + 1);
      div.dataset.feature = m[2];
      div.classList.add("feature");
    }
    div.textContent = text || " ";
    div.addEventListener("click", () => {
      if (div.dataset.feature) selectFeature(div.dataset.feature, false);
    });
    box.appendChild(div);
  });
}

function renderTree(list: Feature[]): void {
  const ul = el("tree");
  ul.textContent = "";
  for (const f of list) {
    const li = document.createElement("li");
    li.dataset.name = f.name;
    li.textContent = `${f.name}  ${f.keyword}`;
    li.title = `${f.faces} faces, ${f.volume_mm3.toFixed(3)} mm³`;
    li.addEventListener("click", () => selectFeature(f.name));
    ul.appendChild(li);
  }
}

async function loadModel(from: string | ArrayBuffer, frame: boolean): Promise<void> {
  const loader = new GLTFLoader();
  const gltf = typeof from === "string" ? await loader.loadAsync(from) : await loader.parseAsync(from, "");
  if (group) scene.remove(group);
  group = gltf.scene;
  faces = [];
  group.traverse((o) => {
    if (!(o instanceof THREE.Mesh)) return;
    const extras = (o.geometry.userData?.id ? o.geometry.userData : o.userData) as {
      id?: string;
      aliases?: string[];
      face?: number;
    };
    o.material = new THREE.MeshStandardMaterial({ color: BASE.clone(), metalness: 0.1, roughness: 0.65 });
    faces.push({ mesh: o, id: extras.id ?? `face ${extras.face ?? "?"}`, aliases: extras.aliases ?? [] });
  });
  scene.add(group);
  if (frame) {
    const box = new THREE.Box3().setFromObject(group);
    const size = box.getSize(new THREE.Vector3()).length() || 1;
    const center = box.getCenter(new THREE.Vector3());
    controls.target.copy(center);
    camera.position.copy(center).add(new THREE.Vector3(0.8, -1.2, 0.7).multiplyScalar(size));
    camera.near = size / 1000;
    camera.far = size * 100;
    camera.updateProjectionMatrix();
    controls.update();
  }
  paint();
}

async function loadPart(file: string, config: string, frame: boolean): Promise<void> {
  const info: PartInfo = await (
    await fetch(`/api/part?file=${encodeURIComponent(file)}&config=${encodeURIComponent(config)}`)
  ).json();
  renderSource(info.source);
  const cfg = el<HTMLSelectElement>("config");
  cfg.textContent = "";
  for (const c of ["", ...info.configs]) cfg.add(new Option(c || "(default)", c));
  cfg.value = config;
  cfg.hidden = info.configs.length === 0;
  const banner = el("banner");
  if (!info.ok) {
    banner.hidden = false;
    banner.textContent = info.error || "The part does not build.";
    features = [];
    renderTree([]);
    return;
  }
  banner.hidden = true;
  features = info.build?.features ?? [];
  renderTree(features);
  await loadModel(`/api/model.glb?file=${encodeURIComponent(file)}&config=${encodeURIComponent(config)}`, frame);
  if (selectedFeature && !features.some((f) => f.name === selectedFeature)) selectFeature(null);
  else paint();
}

function resize(): void {
  const c = renderer.domElement.parentElement as HTMLElement;
  renderer.setSize(c.clientWidth, c.clientHeight, false);
  camera.aspect = c.clientWidth / Math.max(1, c.clientHeight);
  camera.updateProjectionMatrix();
}

const ray = new THREE.Raycaster();
function pick(ev: MouseEvent): void {
  const r = renderer.domElement.getBoundingClientRect();
  ray.setFromCamera(
    new THREE.Vector2(((ev.clientX - r.left) / r.width) * 2 - 1, -((ev.clientY - r.top) / r.height) * 2 + 1),
    camera,
  );
  const hit = ray.intersectObjects(faces.map((f) => f.mesh), false)[0];
  const face = hit ? (faces.find((f) => f.mesh === hit.object) ?? null) : null;
  selectedFeature = face ? featureOf(face.id) : null;
  picked = face;
  paint();
  if (selectedFeature) {
    const line = lineOf.get(selectedFeature);
    if (line !== undefined) {
      document.querySelector(`#source .line[data-line="${line}"]`)?.scrollIntoView({ block: "nearest" });
    }
  }
}

async function poll(): Promise<void> {
  try {
    const v: { version: string } = await (await fetch("/api/version")).json();
    if (current.version && v.version !== current.version && current.file) {
      current.version = v.version;
      await loadPart(current.file, current.config, false);
    }
    current.version = v.version;
  } catch {
    /* the server is gone; keep what is on screen */
  }
}

async function main(): Promise<void> {
  const { parts }: { parts: string[] } = await (await fetch("/api/parts")).json();
  const sel = el<HTMLSelectElement>("part");
  for (const p of parts) sel.add(new Option(p, p));
  sel.addEventListener("change", () => {
    current = { ...current, file: sel.value, config: "" };
    selectedFeature = null;
    void loadPart(current.file, "", true);
  });
  el<HTMLSelectElement>("config").addEventListener("change", (e) => {
    current.config = (e.target as HTMLSelectElement).value;
    void loadPart(current.file, current.config, false);
  });
  renderer.domElement.addEventListener("click", pick);
  window.addEventListener("resize", resize);
  resize();
  renderer.setAnimationLoop(() => renderer.render(scene, camera));
  if (parts.length) {
    current.file = parts[0];
    await loadPart(current.file, "", true);
  }
  await poll();
  setInterval(() => void poll(), 1000);
  // for tests and extensions
  (window as unknown as Record<string, unknown>).__tcad = {
    faces: () => faces.map((f) => ({ id: f.id, aliases: f.aliases })),
    features: () => features.map((f) => f.name),
    selected: () => selectedFeature,
    pickedFace: () => picked?.id ?? null,
    selectFeature: (n: string | null) => selectFeature(n),
    pickFace: (id: string) => {
      picked = faces.find((f) => f.id === id) ?? null;
      selectedFeature = picked ? featureOf(picked.id) : null;
      paint();
    },
    highlightedLines: () =>
      [...document.querySelectorAll<HTMLElement>("#source .line.sel")].map((d) => Number(d.dataset.line)),
    ready: true,
  };
}

// ---- in-browser regeneration (Phase 1b) ----

interface WasmDiagnostic {
  code: string;
  severity: string;
  message: string;
  start: number;
}

interface WasmResult {
  ok: boolean;
  kernel: string;
  configs?: string[];
  features?: Feature[];
  result?: { volume_mm3: number; faces: number } | null;
  diagnostics?: WasmDiagnostic[];
}

let worker: Worker | null = null;
let seq = 0;
let regenerations = 0;
let lastResult: WasmResult | null = null;
let framed = false;
const sources = new Map<number, string>();

function lineAt(src: string, offset: number): number {
  return src.slice(0, offset).split("\n").length;
}

function showStatementInEditor(name: string | null): void {
  const editor = document.getElementById("editor") as HTMLTextAreaElement | null;
  const line = name === null ? undefined : lineOf.get(name);
  if (!editor || line === undefined) return;
  const lines = editor.value.split("\n");
  const start = lines.slice(0, line - 1).reduce((n, l) => n + l.length + 1, 0);
  editor.setSelectionRange(start, start + lines[line - 1].length);
  const lineHeight = editor.scrollHeight / Math.max(1, lines.length);
  editor.scrollTop = Math.max(0, (line - 3) * lineHeight);
}

function regenerate(): void {
  const editor = el<HTMLTextAreaElement>("editor");
  const id = ++seq;
  sources.set(id, editor.value);
  el("status").textContent = "regenerating…";
  worker?.postMessage({ id, src: editor.value, config: current.config });
}

async function onResult(data: { id: number; json: WasmResult; glb: ArrayBuffer | null }): Promise<void> {
  const src = sources.get(data.id) ?? "";
  sources.delete(data.id);
  if (data.id !== seq) return; // a newer edit is already on its way
  const r = data.json;
  lastResult = r;
  const banner = el("banner");
  if (!r.ok) {
    // keep the last good model on screen and say why this text does not build
    banner.hidden = false;
    const errors = (r.diagnostics ?? []).filter((d) => d.severity === "error");
    banner.textContent =
      errors.map((d) => `${d.code} line ${lineAt(src, d.start)}: ${d.message}`).join("\n") || "The part does not build.";
    el("status").textContent = "";
    regenerations++;
    return;
  }
  banner.hidden = true;
  lineOf = scanLines(src);
  const cfg = el<HTMLSelectElement>("config");
  const configs = r.configs ?? [];
  if (cfg.options.length !== configs.length + 1) {
    cfg.textContent = "";
    for (const c of ["", ...configs]) cfg.add(new Option(c || "(default)", c));
    cfg.value = current.config;
  }
  cfg.hidden = configs.length === 0;
  features = r.features ?? [];
  renderTree(features);
  if (data.glb) {
    await loadModel(data.glb, !framed);
    framed = true;
  }
  if (selectedFeature && !features.some((f) => f.name === selectedFeature)) selectedFeature = null;
  paint();
  regenerations++;
}

async function mainBrowser(): Promise<void> {
  el<HTMLSelectElement>("part").hidden = true;
  const editor = document.createElement("textarea");
  editor.id = "editor";
  editor.spellcheck = false;
  editor.setAttribute("aria-label", "Source");
  el("right").replaceChildren(editor);
  const part = new URLSearchParams(location.search).get("part") ?? "example.tcad";
  editor.value = await (await fetch(part)).text();
  worker = new Worker("worker.js");
  worker.onmessage = (e) => void onResult(e.data);
  let timer: ReturnType<typeof setTimeout> | undefined;
  editor.addEventListener("input", () => {
    clearTimeout(timer);
    timer = setTimeout(regenerate, 300);
  });
  // text -> geometry: the statement under the caret selects its feature
  const follow = () => {
    const line = lineAt(editor.value, editor.selectionStart);
    for (const [name, l] of lineOf) {
      if (l === line) {
        selectedFeature = name;
        picked = null;
        paint();
      }
    }
  };
  editor.addEventListener("click", follow);
  editor.addEventListener("keyup", follow);
  el<HTMLSelectElement>("config").addEventListener("change", (e) => {
    current.config = (e.target as HTMLSelectElement).value;
    regenerate();
  });
  renderer.domElement.addEventListener("click", (ev) => {
    pick(ev);
    showStatementInEditor(selectedFeature);
  });
  window.addEventListener("resize", resize);
  resize();
  renderer.setAnimationLoop(() => renderer.render(scene, camera));
  regenerate();
  (window as unknown as Record<string, unknown>).__tcad = {
    faces: () => faces.map((f) => ({ id: f.id, aliases: f.aliases })),
    features: () => features.map((f) => f.name),
    selected: () => selectedFeature,
    pickedFace: () => picked?.id ?? null,
    selectFeature: (n: string | null) => selectFeature(n),
    highlightedLines: () => (selectedFeature && lineOf.has(selectedFeature) ? [lineOf.get(selectedFeature)] : []),
    regenerations: () => regenerations,
    lastResult: () => lastResult,
    setSource: (text: string) => {
      editor.value = text;
      editor.dispatchEvent(new Event("input"));
    },
    ready: true,
  };
}

void (browserMode ? mainBrowser() : main());
