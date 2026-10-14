# CU-21 · Aprender C desde el IDE

**Actor:** Alumno · **Grabación:** [`CU-21-aprender-c.cast`](CU-21-aprender-c.cast) (0:25) · **En la lista de casos:** [CU-21](../casos_de_uso.md#cu-21--aprender-c-desde-el-ide)

## De qué se trata

El menú Ayuda reúne material para aprender: el **glosario** de C (puntero, heap,
segfault, fuga…), las **tablas de consulta** (tipos con su tamaño en esa máquina,
especificadores de printf y scanf, secuencias de escape, precedencia de operadores y
ASCII) y el catálogo de reglas de la cátedra. **Editar → Ver la precedencia de la
expresión** muestra la línea con sus paréntesis implícitos y avisa trampas comunes.

## Qué muestra la grabación

| Tiempo | Paso |
|---|---|
| 0:00 | Ayuda → Glosario de C: se busca un término |
| 0:08 | Ayuda → Tablas de consulta: tipos, printf/scanf, precedencia y ASCII |
| 0:18 | Ver la precedencia de la condición: sus paréntesis implícitos |

## Cómo verla

```bash
asciinema play docs/casos/CU-21-aprender-c.cast        # de corrido
asciinema play -m docs/casos/CU-21-aprender-c.cast     # se detiene en cada paso (Espacio sigue)
```

Las pausas largas (compilar, esperar al programa) se acortan a 2 s al reproducir. La grabación se hizo con el proyecto de demostración de [`grabador/demo`](grabador/demo) en una terminal de 132×36.
