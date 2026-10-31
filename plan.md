# Base rápida de AI Shorter Clip

Sustituir la pantalla de bienvenida por una interfaz en español para seleccionar un vídeo local, previsualizarlo y escribir instrucciones. Esta entrega es un prototipo funcional de interfaz: no incluye IA, subida a servidores ni recorte real.

## Enfoque
- Mantener React + TypeScript + Vite y Tauri 2; usar CSS propio, sin nuevas dependencias.
- Distribución elegida: vídeo a la izquierda, instrucciones a la derecha y resultados de demostración debajo. En ventanas estrechas se apilan.
- Preferir este prototipo a integrar IA/FFmpeg ahora: permite validar rápidamente el flujo sin proveedores, credenciales ni infraestructura.
- Conservar el arranque Linux con DMA-BUF desactivado. No modificar la configuración gráfica ni añadir renderizado por software.

## Interfaz y comportamiento
- Cabecera «AI Shorter Clip», indicador persistente «Modo demo» y aviso «Tu vídeo permanece en este dispositivo. Esta versión no usa IA ni exporta clips».
- Diseño de herramienta: fondo gris claro, texto oscuro, acento azul, tipografía del sistema, divisiones claras y controles accesibles. Sin navegación, estadísticas ni opciones decorativas.
- Selección mediante input de archivo con `accept="video/*"` y botón «Seleccionar vídeo»; permitir sustituirlo o quitarlo. No implementar arrastrar archivos en esta base.
- Mostrar nombre, tamaño y duración cuando se conozca; previsualizar con `<video controls preload="metadata">` usando una URL de objeto, nunca leer el archivo completo en memoria.
- Campo etiquetado «¿Qué clips quieres extraer?» con ejemplo de instrucciones y dos botones que insertan ejemplos: «Momentos destacados» y «Explicaciones breves».
- Acción «Mostrar clips de ejemplo», habilitada solo con vídeo reproducible, duración finita positiva e instrucciones no vacías. No simular análisis ni usar un progreso ficticio.
- Al pulsar, crear hasta tres intervalos deterministas que cubran el vídeo en partes iguales, limitando cada intervalo a 30 segundos y omitiendo partes menores de un segundo. Para vídeos de menos de un segundo, mostrar un único intervalo válido. Sus títulos serán «Ejemplo 1», etc.; indicar explícitamente que no dependen del contenido ni de las instrucciones.
- Resultados con título, inicio/fin y botón «Previsualizar segmento». Reproducir el mismo vídeo desde el inicio elegido y pausarlo al alcanzar el final; al usar manualmente los controles del reproductor se cancela el límite del segmento. Sin botones de descarga o exportación no funcionales.

## Estado y límites
- Estado local React: `VideoSource { file: File; url: string; duration: number | null }`, `instructions: string`, `clips: DemoClip[]`, `activeClipId: string | null` y mensaje de error. `DemoClip { id: string; title: string; startSeconds: number; endSeconds: number }`.
- Sin persistencia: al cerrar o recargar se vuelve al estado vacío. No enviar datos a la red ni registrar el contenido de archivos.
- Revocar URLs al sustituir/quitar el archivo y al desmontar; limpiar resultados, reproducción segmentada y errores al cambiar vídeo. Mantener las instrucciones al sustituirlo; borrar ejemplos al editar instrucciones para evitar resultados obsoletos.
- Rechazar archivos vacíos o claramente no audiovisuales; aceptar extensiones habituales cuando el sistema no facilite MIME. El reproductor es la autoridad final: ante un códec incompatible, mostrar «No se puede reproducir este vídeo. Prueba con un MP4 compatible con tu sistema» y permitir reemplazarlo. No prometer compatibilidad universal con MP4.
- Ignorar eventos tardíos de vídeos sustituidos y manejar rechazos de `play()` sin errores sin capturar.
- Errores y avisos accesibles, etiquetas visibles, foco claro y uso completo mediante teclado. Sin reproducción automática al seleccionar archivo.

## Alcance del cambio
- Reemplazar la demo en `src/App.tsx` y `src/App.css`; ajustar `index.html` a español y al nombre del producto.
- Mantener el backend actual sin añadir comandos, permisos de archivos ni plugins. La selección se realiza mediante el input nativo de la webview.
- Documentar brevemente el flujo y sus límites en `README.md`; registrar los hechos confirmados del producto en `PRODUCT.md` y el diseño finalmente construido en `DESIGN.md`.

## Verificación
- `npm run build`: TypeScript y Vite terminan correctamente.
- `cargo check --manifest-path src-tauri/Cargo.toml`: termina con `Finished` sin errores.
- `npm run tauri dev`: comprobar en el equipo objetivo que la ventana abre sin variables manuales; si continúa el fallo gráfico, reportarlo como bloqueo independiente sin cambiar silenciosamente la solución gráfica.
- Probar: estado vacío, selección/cancelación, vídeo válido, sustitución, eliminación, archivo vacío, códec incompatible, instrucciones vacías y generación repetida. Confirmar intervalos dentro de la duración y previsualización sin crear archivos nuevos.
- Revisar a 800×600 (tamaño actual de Tauri), 1280×800 y 390 px: sin desbordamiento horizontal, formulario legible y todos los controles accesibles. Una revisión visual agrupada y, si hace falta, una ronda de corrección.
- Ejecutar una vez el detector Impeccable sobre los archivos de interfaz modificados y corregir hallazgos mecánicos pertinentes.

## Supuestos y exclusiones
- Uso personal, aplicación de escritorio y texto en español; tema claro como decisión de bajo riesgo para esta base.
- La IA real, transcripción, subtítulos, formatos verticales, exportación, historial, autenticación y procesamiento con FFmpeg quedan fuera de esta entrega.
- Los clips son intervalos ilustrativos sobre el vídeo seleccionado, nunca una afirmación de haber entendido las instrucciones.
