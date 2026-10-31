# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

Interfaz React dentro de una aplicación de escritorio Tauri 2.

## Users

Uso personal: seleccionar un vídeo y describir qué clips se quieren extraer.

## Product Purpose

Facilitar la extracción de clips a partir de instrucciones en lenguaje natural. La entrega actual valida únicamente la interfaz y el flujo local.

## Capabilities and Constraints

- Selección y reproducción de un archivo local; instrucciones editables.
- En escritorio, selector nativo y reproducción directa con libVLC 3 en una superficie nativa; sin servidor HTTP de vídeos ni fotogramas copiados a JavaScript. Controles React y segmentos supervisados en Rust.
- Runtime local validado en Fedora 44/XWayland, sin cambios de paquetes globales. Adaptadores Windows/macOS preparados, pendientes de compilar, probar y empaquetar. No prometer compatibilidad universal comprobada.
- Segmentos de demostración deterministas, no basados en contenido ni instrucciones.
- Sin IA, subidas a servidores, transcripción ni exportación real en esta versión.
- Estado temporal en memoria; no se conserva al cerrar.
- Calidad de la fuente conservada, sin recodificación. Aceleración por hardware en modo automático; la disponibilidad de formatos y GPU depende del runtime, sus módulos y los controladores. En navegador se conserva una vista previa HTML básica.
- Interfaz en español y distribución vídeo/instrucciones lado a lado, apilada en ventanas estrechas.

## Brand Commitments

Nombre de la aplicación: AI Shorter Clip. Modo demo identificado explícitamente.

## Product Principles

- Mantener los archivos locales en este prototipo.
- No presentar resultados ilustrativos como análisis real.
- Priorizar una base sencilla antes de integrar procesamiento e IA.
