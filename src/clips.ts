export type DemoClip = { id: string; title: string; startSeconds: number; endSeconds: number };

export function createDemoClips(duration: number): DemoClip[] {
  if (!Number.isFinite(duration) || duration <= 0) return [];
  const count = Math.min(3, Math.max(1, Math.floor(duration)));
  return Array.from({ length: count }, (_, index) => ({
    id: `example-${index + 1}`,
    title: `Ejemplo ${index + 1}`,
    startSeconds: index * duration / count,
    endSeconds: Math.min(duration, index * duration / count + Math.min(30, duration / count)),
  }));
}

export function formatTime(seconds: number): string {
  if (seconds > 0 && seconds < 1) return `${seconds.toLocaleString("es", { maximumFractionDigits: 2 })} s`;
  const total = Math.floor(seconds);
  const hours = Math.floor(total / 3600);
  const minutes = Math.floor(total / 60) % 60;
  const rest = String(total % 60).padStart(2, "0");
  return hours ? `${hours}:${String(minutes).padStart(2, "0")}:${rest}` : `${minutes}:${rest}`;
}
