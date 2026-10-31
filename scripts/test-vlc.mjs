import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { delimiter, resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const runtime = resolve(root, ".runtime/vlc/usr/lib64");
const env = { ...process.env, AI_SHORTER_TEST_VIDEO: resolve(root, process.argv[2] || "tests/fixtures/test-pattern.webm") };
if (process.platform === "linux" && existsSync(resolve(runtime, "libvlc.so.5"))) {
  env.LD_LIBRARY_PATH = [resolve(runtime, "ffmpeg"), runtime, env.LD_LIBRARY_PATH].filter(Boolean).join(delimiter);
  env.VLC_PLUGIN_PATH = resolve(runtime, "vlc/plugins");
  env.AI_SHORTER_VLC_LIBRARY = resolve(runtime, "libvlc.so.5");
}
const result = spawnSync("cargo", ["test", "--manifest-path", "src-tauri/Cargo.toml", "--lib", "native_playback_lifecycle", "--", "--ignored", "--nocapture"], {
  cwd: root, env, stdio: "inherit", timeout: 180_000,
});
if (result.error) console.error(result.error.message);
process.exit(result.status ?? 1);
