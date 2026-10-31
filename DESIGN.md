# Diseño de AI Shorter Clip

## Superficie y propósito

Herramienta de trabajo para seleccionar un vídeo local y redactar instrucciones. Modo demo visible en todo momento. Distribución aprobada: vídeo a la izquierda, instrucciones a la derecha, segmentos debajo. Sin navegación adicional.

## Sistema construido

- Fondo gris claro `#f4f6f8`, superficie blanca, texto `#202c3d`, secundario `#586575`.
- Azul de acción `#2454c6`; rojo reservado para errores recuperables.
- Tipografía del sistema, títulos compactos y tiempos con números tabulares.
- Contenedor de trabajo con borde fino y radio de 12 px; controles de 7 px. Segmentos en filas, no en tarjetas anidadas.
- Ancho máximo de 1280 px, márgenes fluidos. Dos columnas hasta 680 px, después una columna.
- Sin imágenes remotas, fuentes externas ni dependencias visuales. Iconos SVG pequeños propios.

## Estados e interacción

Selección vacía, metadatos, reproducción lista, error de archivo/códec, instrucciones vacías, ejemplos y segmento activo. Botón principal deshabilitado hasta disponer de vídeo reproducible e instrucciones. Se conserva el vídeo al editar instrucciones, pero se invalidan los resultados. En escritorio, libVLC dibuja el vídeo directamente en una superficie nativa; React mantiene los controles debajo de esa superficie, nunca superpuestos a ella. Controles de reproducción, búsqueda, volumen, silencio y pantalla completa con iconos SVG. Reproducir/pausar o buscar manualmente cancela el límite del segmento; volumen/silencio no. Al previsualizar un segmento, el reproductor entra en pantalla y recibe el foco. La primera imagen se presenta en pausa, sin autoplay de sonido. Al mostrar o redimensionar una superficie pausada se refresca su imagen sin avanzar el tiempo.

## Accesibilidad

Etiquetas explícitas, foco azul visible, mensajes de error con `role="alert"`, número de resultados con región viva y acciones por teclado. El selector es activado por un botón (nativo en Tauri, input de archivo en navegador); la reproducción no comienza al seleccionar archivo. Tras 20 segundos sin estar listo, se muestra un aviso recuperable en lugar de esperar indefinidamente. Transiciones breves únicamente en controles, respetando movimiento reducido.

## Límites de verificación

Interfaz web revisada a 800, 1280 y 390 px. Reproductor nativo comprobado en Tauri/Fedora 44 a 800 y 1280 px, y pantalla completa: primera imagen sin autoplay, MP4 H.264/AAC, metadatos, reproducción, silencio, búsqueda al tercer segmento y pausa al final, retirada y reselección del archivo. LibVLC 3.0.24 utiliza una ventana GTK/X11 mediante XWayland; la sesión del usuario sigue en Wayland. El servidor HTTP de vídeos se ha retirado. La superficie se oculta cuando queda parcial o totalmente fuera del viewport para no pintar sobre otras partes de la interfaz. Los adaptadores Windows/macOS están preparados pero no comprobados; no afirmar compatibilidad validada ni empaquetado completo en esas plataformas. El navegador sigue utilizando vídeo HTML como vista previa básica. Los formatos y la aceleración reales dependen del runtime VLC y sus módulos/controladores.
