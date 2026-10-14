#include <stdio.h>
#include <stdlib.h>

#include "copia.h"

int main(void)
{
    int original[] = {10, 20, 30};
    int *copia = copiar(original, 3);
    if (copia == NULL)
    {
        printf("No hay memoria.\n");
        return 1;
    }
    copia[0] = 99;
    printf("Original: %d · copia: %d\n", original[0], copia[0]);
    free(copia);
    return 0;
}
