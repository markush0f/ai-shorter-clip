# AI Shorter Clip

Aplicación de escritorio Tauri 2 + React + TypeScript. Reproduce vídeos locales con **libVLC 3**, manteniendo la interfaz y los controles en React.

## Desarrollo

Requiere Node.js 22.18+ (24 recomendado), Rust y las dependencias de Tauri. En Linux: GTK 3, WebKitGTK 4.1 y X11/XWayland.

```bash
npm install
# Fedora x86_64: una vez, para preparar el runtime de vídeo local
npm run setup:vlc
npm run tauri dev
```

El instalador local requiere repositorios Fedora y RPM Fusion Free ya configurados, y sus claves en `/etc/pki/rpm-gpg`. Descarga libVLC 3, sus módulos y FFmpeg completo; verifica las firmas RPM usando esas claves en una base de confianza **local**; extrae todo en `.runtime/vlc`. **No usa sudo, no cambia repositorios ni instala/sustituye paquetes del sistema.** Conserva las licencias y la lista de paquetes en `.runtime/vlc/PACKAGES-AND-LICENSES.txt`. Los binarios no se incluyen en Git.

El arranque configura únicamente su propio proceso. Utiliza el runtime local mediante una única reejecución automática y, en Linux, X11/XWayland para la superficie nativa de libVLC 3. Tu sesión puede seguir siendo Wayland. No hay que escribir variables de entorno; se respetan las opciones explícitas `GDK_BACKEND` y `WEBKIT_DISABLE_DMABUF_RENDERER`. Una sesión sin X11/XWayland no está soportada por este adaptador.

### Otros sistemas

- **Linux con libVLC del sistema:** se busca `libvlc.so.5`. Instala también sus módulos de códec y salida de vídeo; el soporte de formatos depende de esa distribución.
- **Windows:** el cargador busca VLC 3 de 64 bits en `%ProgramFiles%/VideoLAN/VLC` o `libvlc.dll` en la ruta del cargador. Hay un adaptador HWND, **pendiente de compilación y validación en Windows**.
- **macOS:** se busca `/Applications/VLC.app/Contents/MacOS/lib/libvlc.dylib`. Hay un adaptador NSView, **pendiente de compilación y validación en macOS**.
- Solo navegador: `npm run dev`. Conserva una vista previa HTML básica, identificada como tal; no utiliza libVLC ni elimina las limitaciones del navegador.

## Reproducción y calidad

- LibVLC lee directamente el archivo autorizado en el selector nativo. No se crea un Blob del vídeo ni se mantiene un servidor HTTP de medios.
- El vídeo se renderiza en una superficie nativa, **sin transportar fotogramas por IPC, base64, canvas ni JavaScript**.
- Decodificación por hardware solicitada en modo automático, con alternativas de VLC según módulos/controladores. No se garantiza que una GPU concreta se utilice en todos los formatos.
- Sin recodificación, reducción de resolución ni filtros artificiales. Se conserva la relación de aspecto; la nitidez está limitada por el archivo original y la pantalla.
- Primera imagen y metadatos en pausa, sin reproducir sonido al seleccionar. Controles de reproducción, búsqueda, volumen, silencio y pantalla completa.
- Solo se envían al frontend estado, tiempos, dimensiones y contadores de fotogramas. Se evita un segundo analizador leyendo el mismo archivo para obtener metadatos.
- Cada selección tiene una sesión aleatoria. Las órdenes atrasadas de un vídeo retirado no controlan el siguiente. Al quitarlo/cambiarlo se liberan sus recursos.

LibVLC no es una garantía de todos los formatos, HDR o aceleración en todos los sistemas. La distribución concreta de bibliotecas, los módulos y los controladores importan.

## Flujo actual: modo demo

1. Selecciona un vídeo local y previsualízalo.
2. Escribe instrucciones o utiliza un ejemplo.
3. **Mostrar clips de ejemplo** genera hasta tres intervalos ilustrativos.
4. **Previsualizar segmento** lleva al reproductor, busca el inicio y reproduce hasta el final del tramo.

El límite del segmento se vigila en Rust, independientemente de la temporización del WebView. Las acciones manuales de reproducción/búsqueda cancelan ese límite; cambiar el volumen o silenciar no lo cancela. La precisión de previsualización depende del reloj y la búsqueda de VLC: **no es un corte garantizado a fotograma**.

**Todavía no hay IA, transcripción, recorte ni exportación real.** Los intervalos no dependen del contenido ni de las instrucciones. El estado es temporal. Los vídeos no se suben ni se modifican; el acceso a metadatos de red de VLC está desactivado. Si no está listo en 20 segundos, se muestra un aviso recuperable.

## Comprobaciones

```bash
npm test
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
cargo check --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
# Requiere el runtime; prueba realmente decodificación y presentación de fotogramas
npm run test:vlc
# Opcional: mismo ciclo con otro archivo local autorizado por el operador
node scripts/test-vlc.mjs /ruta/al/video.mp4
```

Las pruebas normales cubren tiempos, límites y dimensiones de superficie. `test:vlc` ejecuta una prueba separada de metadatos, fotogramas decodificados/presentados, reproducción, pausa, búsqueda, volumen, silencio, segmento y reemplazo/liberación de sesiones. Su fixture sintético es `tests/fixtures/test-pattern.webm`.

Validado en Fedora 44 x86_64, sesión Wayland con ventana XWayland, libVLC 3.0.24: MP4 H.264/AAC del usuario y fixture WebM. Comprobados en Tauri imagen inicial sin autoplay, controles, búsqueda al tercer segmento, pausa final, pantalla completa y retirada/reselección. Hay capturas de comprobación a 800 y 1280 px; los resultados en Chromium no se utilizan para afirmar compatibilidad de códecs nativos.

## Distribución y licencias

**Todavía no se entrega un instalador autocontenido de VLC para los tres sistemas.** El empaquetado y las pruebas por plataforma son trabajo pendiente. En Linux, el arranque también reconoce un directorio `vlc/usr/lib64` junto al ejecutable, con sus módulos y dependencias; un paquete de distribución debe prepararlo explícitamente o declarar dependencias del sistema. No copies el runtime de Fedora a otras distribuciones suponiendo compatibilidad ABI.

LibVLC es LGPL, pero VLC, sus módulos y los builds completos de FFmpeg pueden incorporar componentes GPL y obligaciones adicionales. Antes de publicar instaladores hay que revisar las licencias concretas, avisos, fuentes correspondientes y obligaciones de redistribución de todos los binarios incluidos; no asumir que todo el conjunto tiene una única licencia permisiva.
