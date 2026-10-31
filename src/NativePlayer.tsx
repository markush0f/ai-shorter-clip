import { forwardRef, useEffect, useImperativeHandle, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { formatTime, type DemoClip } from "./clips";

export type NativeSnapshot = {
  id: string; version: string; state: string; duration: number | null; position: number;
  playing: boolean; seekable: boolean; volume: number; muted: boolean;
  width: number; height: number; segmentActive: boolean;
};
type Control = { type: "play" | "pause" } | { type: "seek"; seconds: number } | { type: "volume"; percent: number } | { type: "mute"; muted: boolean };
export type NativePlayerHandle = { pause(): Promise<void>; preview(clip: DemoClip): Promise<void> };
type Props = {
  id: string; name: string;
  onSnapshot(snapshot: NativeSnapshot): void;
  onError(message: string): void;
  onManualInteraction(): void;
  onSegmentEnd(): void;
};

export const NativePlayer = forwardRef<NativePlayerHandle, Props>(function NativePlayer(props, ref) {
  const { id } = props;
  const callbacks = useRef(props);
  callbacks.current = props;
  const screen = useRef<HTMLDivElement>(null);
  const playButton = useRef<HTMLButtonElement>(null);
  const alive = useRef(true);
  const revision = useRef(0);
  const wasSegment = useRef(false);
  const seekTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const [snapshot, setSnapshot] = useState<NativeSnapshot | null>(null);
  const [draftPosition, setDraftPosition] = useState<number | null>(null);
  const [fullscreen, setFullscreen] = useState(false);
  const fullscreenRef = useRef(false);
  const failed = useRef(false);

  function reportError(error: unknown) {
    if (alive.current) callbacks.current.onError(`No se pudo controlar libVLC. ${String(error)}`);
  }
  function accept(value: NativeSnapshot) {
    if (!alive.current || value.id !== id) return;
    setSnapshot(value);
    if (wasSegment.current && !value.segmentActive) callbacks.current.onSegmentEnd();
    wasSegment.current = value.segmentActive;
    callbacks.current.onSnapshot(value);
    if (value.state === "error" && !failed.current) {
      failed.current = true;
      callbacks.current.onError("libVLC no pudo reproducir este archivo. Comprueba que contiene vídeo y que el runtime incluye sus códecs.");
    }
  }
  async function control(command: Control, manual = false) {
    const version = ++revision.current;
    if (manual) callbacks.current.onManualInteraction();
    const value = await invoke<NativeSnapshot>("vlc_control", { id, command });
    if (version === revision.current) accept(value);
  }
  useImperativeHandle(ref, () => ({
    pause: () => control({ type: "pause" }),
    preview: async (clip) => {
      screen.current?.scrollIntoView({ block: "center", behavior: "instant" });
      playButton.current?.focus({ preventScroll: true });
      const version = ++revision.current;
      const value = await invoke<NativeSnapshot>("vlc_preview", { id, start: clip.startSeconds, end: clip.endSeconds });
      if (version === revision.current) accept(value);
    },
  }));

  useEffect(() => {
    alive.current = true;
    let timer: ReturnType<typeof setTimeout>;
    async function poll() {
      const version = revision.current;
      try {
        const value = await invoke<NativeSnapshot>("vlc_status", { id });
        if (!alive.current) return;
        if (version === revision.current) accept(value);
        timer = setTimeout(() => void poll(), value.playing || value.segmentActive ? 150 : 350);
      } catch (error) { reportError(error); }
    }
    void poll();
    return () => {
      alive.current = false;
      clearTimeout(timer);
      if (seekTimer.current) clearTimeout(seekTimer.current);
      if (fullscreenRef.current) void getCurrentWindow().setFullscreen(false).catch(() => {});
    };
  }, [id]);

  useEffect(() => {
    const element = screen.current;
    if (!element) return;
    let frame = 0;
    let previous = "";
    let lastGeometry: { width: number; height: number; visible: boolean; pixelRatio: number } | null = null;
    let disposed = false;
    function syncBounds() {
      const rect = element!.getBoundingClientRect();
      // Hide partly offscreen native surfaces rather than painting over other HTML or window chrome.
      const bounds = { x: rect.left, y: rect.top, width: rect.width, height: rect.height, pixelRatio: window.devicePixelRatio,
        visible: !document.hidden && rect.top >= 0 && rect.left >= 0 && rect.bottom <= innerHeight + 1 && rect.right <= innerWidth + 1 };
      const key = JSON.stringify(bounds);
      if (key === previous) return;
      previous = key;
      const refresh = bounds.visible && (!lastGeometry || !lastGeometry.visible || lastGeometry.width !== bounds.width || lastGeometry.height !== bounds.height || lastGeometry.pixelRatio !== bounds.pixelRatio);
      lastGeometry = bounds;
      void invoke("vlc_surface", { id, bounds: { ...bounds, refresh } }).catch((error) => { if (!disposed) reportError(error); });
    }
    const schedule = () => { cancelAnimationFrame(frame); frame = requestAnimationFrame(syncBounds); };
    const observer = new ResizeObserver(schedule);
    observer.observe(element);
    window.addEventListener("resize", schedule);
    window.addEventListener("scroll", schedule, true);
    document.addEventListener("visibilitychange", schedule);
    schedule();
    return () => {
      disposed = true; observer.disconnect(); cancelAnimationFrame(frame);
      window.removeEventListener("resize", schedule);
      window.removeEventListener("scroll", schedule, true);
      document.removeEventListener("visibilitychange", schedule);
    };
  }, [id]);

  async function toggleFullscreen(next: boolean) {
    await getCurrentWindow().setFullscreen(next);
    if (!alive.current) return;
    fullscreenRef.current = next;
    setFullscreen(next);
  }
  useEffect(() => {
    function escape(event: KeyboardEvent) {
      if (event.key === "Escape" && fullscreenRef.current) void toggleFullscreen(false).catch(reportError);
    }
    document.addEventListener("keydown", escape);
    return () => document.removeEventListener("keydown", escape);
  }, []);

  const duration = snapshot?.duration ?? 0;
  const loading = !snapshot || ["idle", "opening", "buffering"].includes(snapshot.state);
  const run = (command: Control, manual = false) => void control(command, manual).catch(reportError);
  function seek(seconds: number) {
    setDraftPosition(seconds);
    callbacks.current.onManualInteraction();
    if (seekTimer.current) clearTimeout(seekTimer.current);
    seekTimer.current = setTimeout(() => {
      void control({ type: "seek", seconds }).catch(reportError).finally(() => {
        if (alive.current) setDraftPosition(null);
      });
    }, 120);
  }

  return <div className={`native-player${fullscreen ? " native-player--fullscreen" : ""}`} role="group" aria-label={`Reproductor libVLC: ${props.name}`}>
    <div ref={screen} className="native-screen" aria-label="Imagen del vídeo, renderizada directamente por libVLC" />
    <div className="native-controls">
      <div className="native-timeline">
        <span>{formatTime(draftPosition ?? snapshot?.position ?? 0)}</span>
        <input type="range" aria-label="Posición del vídeo" aria-valuetext={`${formatTime(draftPosition ?? snapshot?.position ?? 0)} de ${formatTime(duration)}`} min="0" max={duration || 1} step="0.01" value={Math.min(duration || 1, draftPosition ?? snapshot?.position ?? 0)} disabled={!snapshot?.seekable || !duration} onChange={(event) => seek(Number(event.target.value))} />
        <span>{duration ? formatTime(duration) : "—:—"}</span>
      </div>
      <div className="native-actions">
        <button ref={playButton} className="player-button player-play" aria-label={snapshot?.playing ? "Pausar" : "Reproducir"} disabled={loading || snapshot?.state === "error"} onClick={() => run({ type: snapshot?.playing ? "pause" : "play" }, true)}>
          <svg viewBox="0 0 24 24" aria-hidden="true">{snapshot?.playing ? <path d="M7 5h3v14H7zm7 0h3v14h-3z" /> : <path d="m7 4 14 8-14 8z" />}</svg>
          {snapshot?.playing ? "Pausar" : "Reproducir"}
        </button>
        <button className="player-button" aria-label={snapshot?.muted ? "Activar sonido" : "Silenciar"} disabled={!snapshot} onClick={() => run({ type: "mute", muted: !snapshot?.muted })}>
          <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M3 9h4l5-4v14l-5-4H3z" />{snapshot?.muted ? <path className="icon-stroke" d="m16 9 5 6m0-6-5 6" /> : <path className="icon-stroke" d="M16 8a6 6 0 0 1 0 8m3-11a10 10 0 0 1 0 14" />}</svg>
        </button>
        <input className="native-volume" type="range" aria-label="Volumen" min="0" max="100" step="1" value={snapshot?.volume ?? 100} disabled={!snapshot} onChange={(event) => run({ type: "volume", percent: Number(event.target.value) })} />
        <span className="native-status" role="status">{loading ? "Preparando vídeo…" : snapshot?.segmentActive ? "Segmento en reproducción" : snapshot?.state === "error" ? "Error de reproducción" : "libVLC"}</span>
        <button className="player-button" aria-label={fullscreen ? "Salir de pantalla completa" : "Pantalla completa"} onClick={() => void toggleFullscreen(!fullscreen).catch(reportError)}>
          <svg viewBox="0 0 24 24" aria-hidden="true"><path className="icon-stroke" d="M9 4H4v5m11-5h5v5M4 15v5h5m11-5v5h-5" /></svg>
        </button>
      </div>
    </div>
    <p className="native-engine">libVLC{snapshot ? ` ${snapshot.version.split(" ")[0]}` : ""}{snapshot?.width ? ` · ${snapshot.width} × ${snapshot.height}` : ""} · Sin recodificación</p>
  </div>;
});
