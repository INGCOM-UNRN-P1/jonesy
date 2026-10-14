#include <stdlib.h>

#include "copia.h"

int *copiar(const int v[], size_t n)
{
    int *copia = malloc(n * sizeof(int));
    if (copia == NULL)
    {
        return NULL;
    }
    for (size_t i = 0; i < n; i++)
    {
        copia[i] = v[i];
    }
    return copia;
}
