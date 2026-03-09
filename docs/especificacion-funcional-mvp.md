# Especificación funcional (MVP) - "Escribir mejor en inglés"

## 1) Objetivo
Crear una app de escritorio cross-platform (Tauri) para responder más rápido en Slack/Teams:
- Mejorar texto ya escrito en inglés.
- Dictar en español, transcribir, corregir y traducir al inglés.
- Insertar el resultado en la app activa donde está el cursor.
- Activar la acción desde el menú secundario (click derecho), como disparador principal.
- Permitir un atajo de teclado configurable como disparador alternativo.
- Mantener una app liviana (UI mínima, pocas dependencias y bajo consumo de recursos).

## 2) Alcance del MVP
Incluye:
- Entrada principal desde menú contextual (click derecho) con acción de la app.
- Atajo de teclado configurable por el usuario.
- Panel flotante invocable por atajo global.
- Dos modos: `Improve` y `Dictate+Translate`.
- Edición manual del resultado intermedio (texto o transcripción).
- Inserción por portapapeles + pegado simulado (`Cmd/Ctrl + V`).
- Configuración mínima (idioma origen/destino, tono y atajos).
- Configuración de proveedor de IA y API key por usuario.
- Arquitectura de UI minimalista, evitando librerías de UI pesadas.

No incluye:
- Integraciones nativas con APIs de Slack/Teams.
- Multi-usuario, roles, analytics avanzados.
- Entrenamiento de modelos propios.
- Frameworks visuales grandes o dependencias no esenciales para el MVP.

## 2.1) Principios de implementación liviana
- Base desktop: Tauri (sin Electron).
- Frontend: HTML + CSS + TypeScript, sin framework UI grande.
- Estado local simple (sin Redux/MobX/Zustand en MVP).
- Estilos propios mínimos; evitar design systems pesados.
- Dependencias: solo las necesarias para audio, hotkeys, clipboard e IA.
- Política de librerías: antes de agregar una, justificar impacto en tamaño y mantenimiento.
- Build optimizado para producción (minificación y tree-shaking).

## 3) Perfil de usuario
Profesionales que escriben frecuentemente en inglés en herramientas de chat (Slack, Teams, web chat, email web) y quieren velocidad + claridad.

## 4) Casos de uso principales
1. `Improve`: el usuario pega/escribe texto en inglés y la app devuelve una versión más clara/simple.
2. `Dictate+Translate`: el usuario habla en español, revisa la transcripción y obtiene el mensaje final en inglés.
3. `Insert`: el usuario inserta el texto final en el campo activo sin cambiar de app manualmente.

## 5) Flujos de usuario

### Flujo 0 - Disparo desde menú secundario (click derecho)
1. Usuario selecciona texto en un chat/campo editable.
2. Hace click derecho.
3. Elige acción `Improve English with BestText` en el menú secundario.
4. Se abre panel flotante con el texto pre-cargado en modo `Improve`.
5. Usuario revisa, ejecuta mejora e inserta resultado.

### Flujo A - Improve
1. Usuario activa atajo global de la app.
2. Se abre panel flotante en modo `Improve`.
3. Usuario pega/escribe texto en inglés.
4. Presiona `Improve`.
5. App muestra resultado editable.
6. Usuario presiona `Insertar en cursor`.
7. App copia al portapapeles y simula pegado en ventana activa previa.

### Flujo B - Dictate+Translate
1. Usuario activa atajo global para dictado.
2. Panel abre en modo `Dictate+Translate`.
3. Usuario presiona `Grabar`, habla en español, luego `Detener`.
4. App transcribe y muestra texto editable.
5. Usuario corrige nombres o términos.
6. Presiona `Traducir a inglés`.
7. App muestra versión final editable en inglés.
8. Usuario presiona `Insertar en cursor`.

## 6) Pantallas mínimas
1. **Panel flotante principal**
- Selector de modo (`Improve` / `Dictate+Translate`).
- Área de entrada/salida editable.
- Botones de acción (`Improve`, `Grabar`, `Detener`, `Traducir`, `Insertar`).

2. **Settings básico**
- Toggle: `Preferir menú contextual como entrada`.
- Idioma origen (default: `es-ES`).
- Idioma destino (default: `en-US`).
- Estilo de salida (`simple`, `profesional`, `amigable`).
- Hotkeys configurables.
- Validación de conflictos de hotkeys al guardar.
- Proveedor IA (`OpenAI` o `OpenAI-compatible`).
- `Base URL` configurable (para providers compatibles).
- Modelo por tarea (`Improve` y `Translate`).
- Campo `API key` por proveedor.
- Botón `Test connection`.
- Estado de validación (`Conectado` / `Error`).

## 6.1) Configuración de providers (MVP)
- Provider por default: `OpenAI`.
- Soporte `OpenAI-compatible` vía `Base URL` + `API key` + `model`.
- Permitir guardar múltiples providers y elegir uno activo.
- Si no hay API key válida, deshabilitar acciones IA y mostrar CTA a `Settings`.
- No hardcodear claves ni endpoints privados en código fuente.

## 7) Hotkeys (propuesta inicial)
Uso: fallback cuando el menú contextual no esté disponible en la app destino.

- Abrir app: `Cmd/Ctrl + Shift + Space`
- Abrir en `Improve`: `Cmd/Ctrl + Shift + I`
- Abrir en `Dictate+Translate`: `Cmd/Ctrl + Shift + D`
- Enviar/insertar desde panel: `Cmd/Ctrl + Enter`
- Cerrar panel: `Esc`

Nota: en primera ejecución validar conflictos de atajos con el sistema.
Nota 2: el usuario puede redefinir los atajos desde `Settings`.

## 8) Prompts base (MVP)

### Prompt A - Improve
**System**
"You are a writing assistant. Rewrite the text in clear, natural, concise English. Keep original intent and factual meaning. Avoid slang unless requested."

**User template**
"Style: {style}. Audience: coworker. Return only the improved text.\n\nText:\n{input_text}"

### Prompt B - Translate from Spanish audio
**System**
"You are a translation assistant. Convert Spanish text into clear, natural, concise English suitable for workplace chat. Preserve names and technical terms."

**User template**
"Style: {style}. Return only the final English text.\n\nSpanish text:\n{transcribed_text}"

Regla general:
- Respuesta sin encabezados, sin comillas y sin explicaciones extra.

## 9) Manejo de errores (UX)
0. API key faltante o inválida
- Mensaje: "Configurá una API key válida para continuar."
- Acción: abrir `Settings > Providers`.

1. Micrófono no disponible
- Mensaje: "No se pudo acceder al micrófono. Revisá permisos del sistema."
- Acción: botón `Reintentar` + link a ayuda.

2. Transcripción vacía o baja confianza
- Mensaje: "No pudimos transcribir claramente. Probá grabar de nuevo."
- Acción: `Regrabar`.

3. Falla de red/API
- Mensaje: "No se pudo procesar el texto por un problema de conexión."
- Acción: `Reintentar` y mantener el contenido en pantalla.

4. Inserción fallida en app activa
- Mensaje: "No pudimos pegar automáticamente. El texto quedó copiado."
- Acción: instrucción corta de pegado manual (`Cmd/Ctrl + V`).

## 10) Requisitos no funcionales (MVP)
- Tiempo objetivo de respuesta:
  - `Improve`: <= 3s promedio.
  - `Dictate+Translate`: <= 5s promedio tras detener grabación.
- Privacidad:
  - No guardar audio por defecto.
  - Guardar solo historial de texto local opcional (apagado por default).
  - Guardar API keys en almacenamiento seguro del sistema (Keychain/Credential Manager), no en texto plano.
- Resiliencia:
  - No perder texto si falla una llamada.
- Huella de app:
  - Instalador objetivo <= 35 MB por plataforma (sin contar runtimes del sistema).
  - Inicio de app (cold start) <= 1.5s en equipo de referencia.
  - Consumo en idle <= 180 MB RAM con panel cerrado.
- Compatibilidad:
  - macOS y Windows en primera etapa.
  - El disparo por menú contextual depende del soporte del sistema/app destino; mantener hotkeys como fallback oficial.

## 11) Criterios de aceptación
1. Desde apps compatibles, el click derecho muestra la acción `Improve English with BestText` y abre panel en < 500 ms.
2. `Improve` transforma texto inglés y permite insertar en cursor.
3. `Dictate+Translate` permite grabar, editar transcripción y traducir.
4. Inserción funciona en Slack y Teams en versión desktop/web en al menos 90% de intentos de prueba interna.
5. En caso de error, el usuario recibe mensaje claro y opción de reintento sin perder contenido.
6. El usuario puede cambiar al menos los atajos de `Abrir app` y `Abrir Dictate+Translate`, y el nuevo atajo queda aplicado sin reiniciar.
7. El build de producción del MVP no incluye librerías de UI pesadas y cumple metas de huella (instalador/startup/idle RAM).
8. El usuario puede configurar al menos `OpenAI` y un provider `OpenAI-compatible` (Base URL + API key + modelo), validar conexión y usarlo en ambos modos.

## 12) Próximo paso técnico
Implementar un vertical slice end-to-end:
1. Tauri + panel flotante + hotkey global.
2. `Improve` funcionando con proveedor LLM.
3. Inserción por clipboard + pegado simulado.
