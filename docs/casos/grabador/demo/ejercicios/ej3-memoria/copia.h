#ifndef COPIA_H
#define COPIA_H

#include <stddef.h>

/* Una copia de los `n` elementos de `v` en memoria dinámica (NULL si no hay memoria).
 * Quien la pide la libera con free. */
int *copiar(const int v[], size_t n);

#endif
