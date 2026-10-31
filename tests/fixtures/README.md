# Fixture de vídeo

`test-pattern.webm` es un patrón sintético de seis segundos, 160×90 a 24 fps, con audio estéreo silencioso. No contiene imágenes ni audio del usuario.

Generación con FFmpeg (VP8 + Vorbis):

```bash
ffmpeg -f lavfi -i testsrc2=size=160x90:rate=24 \
  -f lavfi -i anullsrc=r=48000:cl=stereo -t 6 \
  -c:v libvpx -b:v 150k -c:a libvorbis -q:a 2 test-pattern.webm
```

Lo utiliza `npm run test:vlc`. La prueba comprueba contadores de fotogramas, no solo que avance el reloj.
