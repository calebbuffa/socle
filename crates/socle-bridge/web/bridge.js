// socle-bridge Babylon.js client
// Connects to ws://localhost:9001, sends camera state, receives GLB tiles.
//
// Coordinate convention:
//   Rust/ECEF: Z-up, right-handed
//   Babylon:   Y-up, left-handed
// We work in ECEF throughout and just swap Y↔Z when talking to Babylon APIs.

const WS_URL = "ws://127.0.0.1:9001";
const SEND_HZ = 30; // camera updates per second

// ── Coordinate helpers ───────────────────────────────────────────────────────

// ECEF (Z-up RH) → Babylon (Y-up LH): swap Y and Z
function ecefToBabylon(x, y, z) { return new BABYLON.Vector3(x, z, y); }
// Babylon (Y-up LH) → ECEF (Z-up RH): swap Y and Z back
function babylonToEcef(v) { return [v.x, v.z, v.y]; }

// Convert a column-major 4×4 ECEF matrix to a Babylon world matrix.
// The ECEF transform M maps Z-up local vertices to Z-up world positions.
// Babylon is Y-up, and our GLB vertices are also Z-up (not Y-up as glTF expects).
// So we need: result = S * M  where S swaps Y↔Z.
// This swaps the world output to Y-up AND implicitly fixes the Z-up local verts
// (S*M*S would double-swap the input, but since GLB verts are already Z-up and
//  Babylon reads them as-is, we only need one S on the left).
function ecefMatrixToBabylon(cols) {
    const c = cols;
    const m = new Float32Array(16);
    // S * M in column-major: swap rows 1↔2 of M.
    // col j of M = [c[4j], c[4j+1], c[4j+2], c[4j+3]] (rows 0,1,2,3)
    // After row swap 1↔2: [c[4j], c[4j+2], c[4j+1], c[4j+3]]
    m[0]=c[0]; m[1]=c[2]; m[2]=c[1]; m[3]=c[3];
    m[4]=c[4]; m[5]=c[6]; m[6]=c[5]; m[7]=c[7];
    m[8]=c[8]; m[9]=c[10]; m[10]=c[9]; m[11]=c[11];
    m[12]=c[12]; m[13]=c[14]; m[14]=c[13]; m[15]=c[15];
    return BABYLON.Matrix.FromArray(m);
}

// ── Babylon setup ────────────────────────────────────────────────────────────

const canvas = document.getElementById("renderCanvas");
const engine = new BABYLON.Engine(canvas, true, { preserveDrawingBuffer: true, stencil: true });
const scene = new BABYLON.Scene(engine);
scene.clearColor = new BABYLON.Color4(0.02, 0.02, 0.05, 1);

// Arc-rotate camera — positioned in Babylon coords (Y-up).
// Start above the equator at ~2e7 m altitude, looking at origin.
const camera = new BABYLON.ArcRotateCamera(
    "cam",
    -Math.PI / 2, Math.PI / 4, 2.5e7,
    BABYLON.Vector3.Zero(),
    scene
);
camera.minZ = 1000;      // adapted per-frame below; start conservative
camera.maxZ = 1e9;
camera.wheelPrecision = 0.0001;
camera.panningSensibility = 50;
camera.lowerRadiusLimit = 6.4e6 + 100; // don't go below surface
camera.attachControl(canvas, true);

// Adapt minZ every frame so the depth range stays tight:
// minZ = max(1m, altitude_above_surface * 0.001).
// This keeps the 24-bit depth buffer useful at all altitudes.
scene.onBeforeRenderObservable.add(() => {
    const alt = Math.max(0, camera.radius - 6.371e6);
    camera.minZ = Math.max(1, alt * 0.001);
});

// Hemispheric light
const light = new BABYLON.HemisphericLight("light", new BABYLON.Vector3(0, 1, 0), scene);
light.intensity = 1.0;

// ── Tile management ──────────────────────────────────────────────────────────

// Map node-id → { glbData: Uint8Array | null, meshes: AbstractMesh[], transform: Float64Array }
const tiles = new Map();

function setTileTransform(id, transformCols) {
    const entry = tiles.get(id);
    if (!entry || !entry.meshes.length) return;

    const m = ecefMatrixToBabylon(transformCols);
    // Debug: log first tile transform
    if (!setTileTransform._logged) {
        setTileTransform._logged = true;
        console.log("ECEF cols:", transformCols);
        console.log("Babylon matrix:", Array.from(m.m));
        const t = m.getTranslation();
        console.log("Translation:", t.x, t.y, t.z, "dist:", t.length());
    }
    for (const mesh of entry.meshes) {
        mesh.freezeWorldMatrix(m);
    }
}

function setTileAlpha(id, alpha) {
    const entry = tiles.get(id);
    if (!entry) return;
    for (const mesh of entry.meshes) {
        if (!mesh.material) continue;
        mesh.material.alpha = alpha;
        // Enable transparency mode when fading so the depth sort works correctly.
        mesh.material.transparencyMode = alpha < 1.0
            ? BABYLON.Material.MATERIAL_ALPHABLEND
            : BABYLON.Material.MATERIAL_OPAQUE;
    }
}

// Track tiles that have been explicitly removed so we can distinguish
// "not yet added" from "removed while loading".
const removedTiles = new Set();
// Track the generation (load counter) per tile id so in-flight loads that
// are superseded by a newer binary can be discarded on completion.
const pendingGeneration = new Map();

async function loadGlb(id, glbData) {
    // Bump generation for this id so any in-flight load for a previous binary
    // knows it has been superseded.
    const gen = (pendingGeneration.get(id) || 0) + 1;
    pendingGeneration.set(id, gen);

    try {
        // Dispose any existing meshes for this tile (e.g. overlay re-bake).
        const existing = tiles.get(id);
        if (existing && existing.meshes.length) {
            for (const mesh of existing.meshes) {
                mesh.dispose();
            }
            existing.meshes = [];
        }

        // Ensure an entry exists so frame messages can attach transforms.
        if (!tiles.has(id)) {
            tiles.set(id, { glbData: null, meshes: [], transform: null });
        }
        removedTiles.delete(id);

        // Create a Blob URL so Babylon's glTF loader can fetch it properly
        const blob = new Blob([glbData], { type: "model/gltf-binary" });
        const url = URL.createObjectURL(blob);
        const result = await BABYLON.SceneLoader.ImportMeshAsync("", url, "", scene, null, ".glb");
        URL.revokeObjectURL(url);

        // If a newer load for this id has started, discard this result.
        if (pendingGeneration.get(id) !== gen) {
            for (const mesh of result.meshes) mesh.dispose();
            return;
        }

        // If the tile was removed while we were loading, dispose immediately.
        if (removedTiles.has(id)) {
            for (const mesh of result.meshes) mesh.dispose();
            removedTiles.delete(id);
            return;
        }

        const entry = tiles.get(id) || { glbData: null, meshes: [], transform: null, alpha: 1.0 };
        entry.glbData = glbData;
        entry.meshes = result.meshes;
        // Disable back-face culling — the Y↔Z swap reverses winding order.
        // Enable logarithmic depth to eliminate Z-fighting at planetary scale.
        for (const mesh of entry.meshes) {
            if (mesh.material) {
                mesh.material.backFaceCulling = false;
                mesh.material.useLogarithmicDepth = true;
            }
        }
        tiles.set(id, entry);
        console.log(`tile ${id}: loaded ${entry.meshes.length} meshes`);

        // Apply transform and alpha if we already have them.
        if (entry.transform) {
            setTileTransform(id, entry.transform);
        }
        setTileAlpha(id, entry.alpha ?? 1.0);
    } catch (e) {
        console.warn(`failed to load GLB for tile ${id}:`, e);
    }
}

function arrayBufferToBase64(buffer) {
    let binary = '';
    const bytes = new Uint8Array(buffer);
    for (let i = 0; i < bytes.byteLength; i++) {
        binary += String.fromCharCode(bytes[i]);
    }
    return btoa(binary);
}

function removeTile(id) {
    removedTiles.add(id);
    const entry = tiles.get(id);
    if (entry) {
        for (const mesh of entry.meshes) {
            mesh.dispose();
        }
        tiles.delete(id);
    }
}

// ── WebSocket ────────────────────────────────────────────────────────────────

const statusEl = document.getElementById("status");
let ws = null;
let connected = false;

function connect() {
    ws = new WebSocket(WS_URL);
    ws.binaryType = "arraybuffer";

    ws.onopen = () => {
        connected = true;
        statusEl.textContent = "connected";
        console.log("ws connected");
    };

    ws.onclose = () => {
        connected = false;
        statusEl.textContent = "disconnected — reconnecting…";
        setTimeout(connect, 2000);
    };

    ws.onerror = (e) => {
        console.error("ws error", e);
    };

    ws.onmessage = (evt) => {
        if (typeof evt.data === "string") {
            // JSON frame message
            const msg = JSON.parse(evt.data);
            if (msg.type === "frame") {
                // Update transforms and alpha for added tiles.
                for (const add of msg.add) {
                    let entry = tiles.get(add.id);
                    if (!entry) {
                        entry = { glbData: null, meshes: [], transform: null, alpha: 1.0 };
                        tiles.set(add.id, entry);
                    }
                    entry.transform = add.transform;
                    entry.alpha = add.alpha ?? 1.0;

                    // Show/hide: make sure meshes are enabled.
                    for (const mesh of entry.meshes) {
                        mesh.setEnabled(true);
                    }
                    setTileTransform(add.id, add.transform);
                    setTileAlpha(add.id, entry.alpha);
                }

                // Remove tiles no longer visible.
                for (const id of msg.remove) {
                    removeTile(id);
                }

                statusEl.textContent = `tiles: ${tiles.size} | add: ${msg.add.length} | rm: ${msg.remove.length}`;
            }
        } else {
            // Binary: 4-byte LE node-id + GLB
            const buf = new Uint8Array(evt.data);
            const id = new DataView(evt.data).getUint32(0, true);
            const glbData = buf.slice(4);
            console.log(`received GLB for tile ${id}, ${glbData.byteLength} bytes`);
            loadGlb(id, glbData);
        }
    };
}

connect();

// ── Camera → server updates ─────────────────────────────────────────────────

function getCameraState() {
    // Convert Babylon camera (Y-up) to ECEF (Z-up) for the Rust backend.
    const pos = camera.position;
    const target = camera.target;
    const dir = target.subtract(pos).normalize();
    const up = camera.upVector;

    // Swap Y↔Z to go from Babylon → ECEF
    const ecefPos = babylonToEcef(pos);
    const ecefDir = babylonToEcef(dir);
    const ecefUp = babylonToEcef(up);

    return {
        type: "view",
        position: ecefPos,
        direction: ecefDir,
        up: ecefUp,
        viewport: [canvas.width, canvas.height],
        // Correct horizontal FOV: 2 * atan(tan(fov_y/2) * aspectRatio)
        fov_x: 2 * Math.atan(Math.tan(camera.fov / 2) * engine.getAspectRatio(camera)),
        fov_y: camera.fov,
    };
}

let sendInterval = null;
function startSending() {
    if (sendInterval) return;
    sendInterval = setInterval(() => {
        if (connected && ws.readyState === WebSocket.OPEN) {
            ws.send(JSON.stringify(getCameraState()));
        }
    }, 1000 / SEND_HZ);
}

// Start sending once scene is ready.
scene.onReadyObservable.addOnce(() => {
    startSending();
});

// ── Render loop ──────────────────────────────────────────────────────────────

engine.runRenderLoop(() => {
    scene.render();
});

window.addEventListener("resize", () => {
    engine.resize();
});
