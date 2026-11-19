# Parche de winit 0.30.13 para Android (cal3-rust)

Cambios en `src/platform_impl/android/mod.rs` (buscar «PARCHE cal3-rust»):

1. Emite `WindowEvent::ModifiersChanged` deduciendo Ctrl/Mayús/Alt/Meta del
   `meta_state` de cada tecla (winit no los soporta en Android), para que
   funcionen Ctrl+A, Ctrl+Z, Mayús+flechas, etc. con teclado físico.
2. El botón «Atrás» se entrega como `Escape`.
