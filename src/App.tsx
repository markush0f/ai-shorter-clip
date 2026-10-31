import { useEffect, useRef, useState } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { stat } from "@tauri-apps/plugin-fs";
import { createDemoClips, formatTime, type DemoClip } from "./clips";
import { NativePlayer, type NativePlayerHandle, type NativeSnapshot } from "./NativePlayer";
import "./App.css";

type VideoSource = { file: Pick<File, "name" | "size">; url: string; duration: number | null; native?: boolean };
const examples = {
  "Momentos destacados": "Extrae los momentos más interesantes en clips de hasta 30 segundos. Conserva el contexto de cada momento.",
  "Explicaciones breves": "Busca explicaciones claras que se entiendan por sí solas y conviértelas en clips breves.",
};

function App() {
  const [source, setSource] = useState<VideoSource | null>(null);
  const [instructions, setInstructions] = useState("");
  const [clips, setClips] = useState<DemoClip[]>([]);
  const [activeClipId, setActiveClipId] = useState<string | null>(null);
  const [error, setError] = useState("");
  const [ready, setReady] = useState(false);
  const [selecting, setSelecting] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);
  const videoRef = useRef<HTMLVideoElement>(null);
  const nativeRef = useRef<NativePlayerHandle>(null);
  const currentUrl = useRef<string | null>(null);
  const segmentEnd = useRef<number | null>(null);

  const sourceUrl = source?.url;
  const nativeSource = source?.native;
  useEffect(() => () => {
    if (sourceUrl?.startsWith("blob:")) URL.revokeObjectURL(sourceUrl);
    else if (sourceUrl && nativeSource) void invoke("vlc_unload", { id: sourceUrl }).catch(() => {});
  }, [sourceUrl, nativeSource]);

  useEffect(() => {
    if (!sourceUrl || ready || error) return;
    const timer = window.setTimeout(() => {
      if (currentUrl.current === sourceUrl) {
        setError("El vídeo no ha terminado de cargar. Puedes cambiarlo o volver a seleccionar el mismo archivo para reintentar. Es posible que su códec no sea compatible con este sistema.");
      }
    }, 20000);
    return () => window.clearTimeout(timer);
  }, [sourceUrl, ready, error]);

  function cancelSegment() {
    segmentEnd.current = null;
    setActiveClipId(null);
  }

  function resetResults() {
    if (segmentEnd.current !== null) {
      videoRef.current?.pause();
      void nativeRef.current?.pause().catch(() => {});
    }
    cancelSegment();
    setClips([]);
  }

  function clearSource() {
    videoRef.current?.pause();
    void nativeRef.current?.pause().catch(() => {});
    currentUrl.current = null;
    setSource(null);
    setReady(false);
    setError("");
    resetResults();
  }

  async function chooseVideo() {
    if (!isTauri()) { inputRef.current?.click(); return; }
    setSelecting(true);
    try {
      const path = await open({
        title: "Seleccionar vídeo",
        multiple: false,
        directory: false,
        filters: [{ name: "Vídeos", extensions: ["mp4", "m4v", "mov", "webm", "ogv", "mkv", "avi", "ts", "mts", "m2ts", "wmv", "mpg", "mpeg", "flv", "vob"] }],
      });
      if (!path) return;
      const metadata = await stat(path);
      if (!metadata.isFile || !metadata.size) {
        setError("El archivo está vacío o no es un vídeo válido. Selecciona otro archivo.");
        return;
      }
      clearSource();
      const player = await invoke<NativeSnapshot>("vlc_load", { path });
      currentUrl.current = player.id;
      setSource({ file: { name: path.split(/[\\/]/).pop() || "Vídeo local", size: metadata.size }, url: player.id, duration: player.duration, native: true });
    } catch (error) {
      setError(`No se pudo abrir el vídeo local. ${String(error)}`);
    } finally {
      setSelecting(false);
    }
  }

  function selectFile(file?: File) {
    if (!file) return;
    if (!file.size || !(file.type.startsWith("video/") || (!file.type && /\.(mp4|m4v|mov|webm|ogv|ogg|mkv|avi)$/i.test(file.name)))) {
      setError(!file.size ? "El archivo está vacío. Selecciona otro vídeo." : "Selecciona un archivo de vídeo válido.");
      return;
    }
    clearSource();
    const url = URL.createObjectURL(file);
    currentUrl.current = url;
    setSource({ file, url, duration: null });
  }

  function changeInstructions(value: string) {
    setInstructions(value);
    resetResults();
  }

  function playbackError(url: string) {
    if (currentUrl.current !== url) return;
    setReady(false);
    resetResults();
    setError("El navegador no puede reproducir este formato. Abre la aplicación de escritorio para utilizar libVLC.");
  }

  async function previewClip(clip: DemoClip) {
    const video = videoRef.current;
    const url = currentUrl.current;
    if (!url) return;
    if (source?.native) {
      setError("");
      segmentEnd.current = clip.endSeconds;
      setActiveClipId(clip.id);
      try { await nativeRef.current?.preview(clip); }
      catch (error) {
        if (currentUrl.current !== url) return;
        cancelSegment();
        setError(`No se pudo previsualizar el segmento. ${String(error)}`);
      }
      return;
    }
    if (!video) return;
    setError("");
    try {
      segmentEnd.current = clip.endSeconds;
      setActiveClipId(clip.id);
      video.currentTime = clip.startSeconds;
      await video.play();
    } catch {
      if (currentUrl.current !== url) return;
      cancelSegment();
      setError("No se pudo iniciar la reproducción. Prueba con los controles del vídeo.");
    }
  }

  const canGenerate = ready && source?.duration != null && source.duration > 0 && Boolean(instructions.trim());

  return (
    <main className="app">
      <header className="app-header">
        <div className="brand"><svg aria-hidden="true" viewBox="0 0 32 32"><rect x="3" y="5" width="26" height="22" rx="5" /><path d="m13 11 9 5-9 5z" /></svg><span>AI Shorter Clip</span></div>
        <span className="demo-badge">Modo demo</span>
      </header>

      <div className="intro"><h1>De un vídeo a tus mejores clips.</h1><p>Elige tu vídeo y cuéntanos qué quieres encontrar.</p></div>
      <p className="privacy">Tu vídeo permanece en este dispositivo. Esta versión no usa IA ni exporta clips.</p>

      <div className="workspace">
        <section className="video-section" aria-labelledby="video-heading">
          <div className="section-heading"><h2 id="video-heading">Tu vídeo</h2>{source && <button className="text-button" onClick={clearSource}>Quitar vídeo</button>}</div>
          <input ref={inputRef} className="file-input" type="file" accept="video/*" aria-label="Seleccionar vídeo" onChange={(event) => { selectFile(event.target.files?.[0]); event.target.value = ""; }} />
          {source ? (
            <>
              {source.native ? <NativePlayer key={source.url} ref={nativeRef} id={source.url} name={source.file.name}
                onSnapshot={(snapshot) => {
                  if (currentUrl.current !== snapshot.id) return;
                  const available = ["playing", "paused", "ended"].includes(snapshot.state) && snapshot.duration !== null;
                  if (available && !ready) setError("");
                  setReady(available);
                  if (snapshot.duration !== null) setSource((value) => value?.url === snapshot.id && value.duration !== snapshot.duration ? { ...value, duration: snapshot.duration } : value);
                }}
                onError={(message) => { if (currentUrl.current === source.url) { setReady(false); setError(message); } }}
                onManualInteraction={cancelSegment} onSegmentEnd={cancelSegment}
              /> : <video key={source.url} ref={videoRef} src={source.url} controls preload="auto" aria-label={`Vista previa de ${source.file.name}`}
                onLoadedMetadata={(event) => {
                  if (currentUrl.current !== source.url) return;
                  const duration = event.currentTarget.duration;
                  // WebKit may initially report Infinity for a seekable MP4.
                  // It is not a codec error; durationchange supplies the final value.
                  if (Number.isFinite(duration) && duration > 0) {
                    setSource((value) => value?.url === source.url ? { ...value, duration } : value);
                  }
                }}
                onDurationChange={(event) => {
                  const duration = event.currentTarget.duration;
                  if (currentUrl.current === source.url && Number.isFinite(duration) && duration > 0) {
                    setSource((value) => value?.url === source.url ? { ...value, duration } : value);
                  }
                }}
                onCanPlay={() => {
                  if (currentUrl.current === source.url) { setReady(true); setError(""); }
                }}
                onError={() => playbackError(source.url)}
                onPointerDown={cancelSegment} onKeyDown={cancelSegment}
                onTimeUpdate={(event) => {
                  if (currentUrl.current !== source.url) return;
                  const duration = event.currentTarget.duration;
                  if (Number.isFinite(duration) && duration > 0 && source.duration !== duration) {
                    setSource((value) => value?.url === source.url ? { ...value, duration } : value);
                  }
                  if (segmentEnd.current !== null && event.currentTarget.currentTime >= segmentEnd.current) {
                    event.currentTarget.pause();
                    event.currentTarget.currentTime = segmentEnd.current;
                    cancelSegment();
                  }
                }} onEnded={cancelSegment} />}
              <div className="file-details"><div><p className="filename" title={source.file.name}>{source.file.name}</p><p className="file-meta">{source.file.size < 1024 * 1024 ? `${Math.max(1, Math.round(source.file.size / 1024))} KB` : `${(source.file.size / 1024 / 1024).toLocaleString("es", { maximumFractionDigits: 1 })} MB`} · {source.duration !== null ? formatTime(source.duration) : ready ? "Inicia la reproducción para leer la duración" : "Leyendo duración…"}</p></div><button className="secondary-button" disabled={selecting} onClick={() => void chooseVideo()}>Cambiar vídeo</button></div>
            </>
          ) : (
            <div className="upload-area">
              <svg className="upload-icon" aria-hidden="true" viewBox="0 0 48 48"><rect x="6" y="10" width="36" height="28" rx="4" /><path d="m20 18 11 6-11 6zM6 17h6M6 31h6M36 17h6M36 31h6" /></svg>
              <h3>Todo empieza con un vídeo</h3><p>Selecciona un archivo de tu dispositivo<br />para verlo aquí.</p><button className="secondary-button" disabled={selecting} onClick={() => void chooseVideo()}>{selecting ? "Abriendo selector…" : "Seleccionar vídeo"}</button><small>{isTauri() ? "Reproducción nativa con libVLC, sin convertir tu archivo." : "Vista previa básica. La aplicación de escritorio utiliza libVLC."}</small>
            </div>
          )}
        </section>

        <section className="instructions-section" aria-labelledby="instructions-heading">
          <h2 id="instructions-heading">Dale una dirección</h2>
          <label htmlFor="instructions">¿Qué clips quieres extraer?</label>
          <textarea id="instructions" value={instructions} onChange={(event) => changeInstructions(event.target.value)} placeholder="Por ejemplo: busca consejos prácticos que se entiendan sin ver el vídeo completo…" aria-describedby="instructions-help" />
          <p id="instructions-help" className="field-help">Describe el tema, el estilo o los momentos que te interesan.</p>
          <div className="example-buttons">{Object.entries(examples).map(([label, value]) => <button key={label} className="example-button" onClick={() => changeInstructions(value)}>{label}</button>)}</div>
          <div className="generate-area"><button className="primary-button" disabled={!canGenerate} onClick={() => { resetResults(); setClips(createDemoClips(source!.duration!)); }}>Mostrar clips de ejemplo <span aria-hidden="true">→</span></button><p>Son intervalos ilustrativos, no una selección de IA.</p></div>
        </section>
      </div>

      {error && <p className="error-message" role="alert">{error}</p>}
      <section className="results" aria-labelledby="results-heading">
        <div className="results-heading"><h2 id="results-heading">Clips de ejemplo</h2><span aria-live="polite">{clips.length ? `${clips.length} segmentos ilustrativos` : "Sin segmentos todavía"}</span></div>
        {clips.length ? <><p className="results-note">Estos intervalos no dependen del contenido del vídeo ni de tus instrucciones. No se han creado archivos nuevos.</p><ol className="clip-list">{clips.map((clip) => <li key={clip.id} className={activeClipId === clip.id ? "clip-row active" : "clip-row"}><div className="clip-title"><h3>{clip.title}</h3><span>{formatTime(clip.endSeconds - clip.startSeconds)} de duración</span></div><p className="time-range">{formatTime(clip.startSeconds)} — {formatTime(clip.endSeconds)}</p><button className="secondary-button" onClick={() => void previewClip(clip)} aria-label={`Previsualizar segmento: ${clip.title}`}>{activeClipId === clip.id ? "Reiniciar segmento" : "Previsualizar segmento"}</button></li>)}</ol></> : <div className="empty-results"><p>Aquí podrás revisar tus segmentos.</p><span>Selecciona un vídeo, escribe tus instrucciones y muestra los ejemplos.</span></div>}
      </section>
      <footer>Una primera base para dar forma a tus ideas. Sin subidas, sin procesamiento de IA.</footer>
    </main>
  );
}

export default App;
