# Molex

Molex es una aplicación de escritorio remoto de latencia ultrabaja diseñada para controlar entornos Linux (Wayland) desde un dispositivo Android. 

Su arquitectura de renderizado divide la carga de procesamiento en dos flujos independientes, optimizando el ancho de banda y garantizando latencia cero (fire-and-forget) para interacciones del usuario.

## Arquitectura del Backend (Motor Nativo en Rust)
El backend está implementado en Rust y se integra a la aplicación Android mediante `UniFFI`. Está segmentado en dos clientes especializados:

1. **`MolexVideoClient` (Vía SSH)**
   - Encargado de la telemetría y renderizado del video.
   - Utiliza un algoritmo de compresión y un sistema de optimización tipo LTPO que envía la cadena `"SAME_FRAME"` si no ha habido cambios en la pantalla, ahorrando recursos de CPU, RAM y ancho de banda.
   - Recupera métricas del sistema hospedador (OS, RAM, CPU).
   
2. **`MolexInputClient` (Vía UDP)**
   - Responsable de inyectar eventos de hardware (ratón y teclado) directamente al Kernel del host.
   - Comunicación de latencia cero gracias a su naturaleza UDP y *fire-and-forget*.

## Estado Actual del Proyecto
- [x] Motor nativo en Rust implementado y optimizado.
- [x] Puente JNI/UniFFI completado (`uniffi.client`).
- [x] Arquitectura de Presentación (`MolexViewModel`) diseñada (separación estricta entre flujos IO y la capa UI para evitar el bloqueo del Hilo Principal de Compose).
- [ ] Implementación en código del `MolexViewModel`.
- [ ] Desarrollo de la UI en Jetpack Compose (`RemoteScreen` y `MetricsOverlay`).
- [ ] Mapeo de gestos táctiles (Drag & Tap) y conversión a porcentajes (0.0 a 1.0) para el servidor.

## Próximos Pasos (En Desarrollo)
Estamos enfocados en construir la capa UI/UX en Jetpack Compose:
1. Instanciar y configurar `MolexViewModel` mediante Corrutinas (`Dispatchers.IO`).
2. Diseñar la pantalla principal a pantalla completa para el despliegue fluido de los fotogramas (Base64 -> ImageBitmap).
3. Integrar los modificadores `pointerInput` en Compose para capturar y traducir de manera asíncrona todos los eventos de interacción.
4. Crear un componente overlay semi-transparente y oscuro que visualice las métricas de hardware del servidor.