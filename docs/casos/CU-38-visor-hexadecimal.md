# CU-38 · Mirar por dentro un ejecutable, un .o o un .a

**Actor:** Alumno · **Grabación:** [`CU-38-visor-hexadecimal.cast`](CU-38-visor-hexadecimal.cast) (0:31) · **En la lista de casos:** [CU-38](../casos_de_uso.md#cu-38--mirar-por-dentro-un-ejecutable-un-o-o-un-a)

## De qué se trata

Los binarios (ejecutables, objetos `.o`, bibliotecas `.a`) se abren en una pestaña con el
**visor hexadecimal**: offset, bytes y ASCII, con el byte del cursor en hexadecimal,
decimal y carácter, y los 4 bytes desde ahí leídos como `int`. `g` (o Ctrl+G) va a un
offset y `/` (o Ctrl+F) busca texto o bytes. `i` describe el archivo (tipo, arquitectura,
las secciones del ELF explicadas o los objetos de un `.a`) y `s` muestra los símbolos con
`nm`: las funciones que define (T) y las que usa de otro lado (U), lo que explica un
«undefined reference». El archivo nunca se modifica.

## Qué muestra la grabación

| Tiempo | Paso |
|---|---|
| 0:00 | F6 compila: quedan el ejecutable, los .o y la biblioteca .a |
| 0:02 | Ver → Visor hexadecimal lista los binarios del proyecto |
| 0:11 | / busca un texto dentro del ejecutable |
| 0:14 | i describe el archivo: tipo y secciones del ELF explicadas |
| 0:19 | s muestra los símbolos: lo que define (T) y lo que usa de otro lado (U) |
| 0:22 | La biblioteca libvector.a: un paquete de objetos .o |
| 0:29 | Ctrl+W cierra la pestaña |

## Cómo verla

```bash
asciinema play docs/casos/CU-38-visor-hexadecimal.cast        # de corrido
asciinema play -m docs/casos/CU-38-visor-hexadecimal.cast     # se detiene en cada paso (Espacio sigue)
```

Las pausas largas (compilar, esperar al programa) se acortan a 2 s al reproducir. La grabación se hizo con el proyecto de demostración de [`grabador/demo`](grabador/demo) en una terminal de 132×36.
