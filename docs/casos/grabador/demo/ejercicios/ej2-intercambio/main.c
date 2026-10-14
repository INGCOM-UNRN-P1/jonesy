#include <stdio.h>

#include "intercambio.h"

int main(void)
{
    int primero = 3;
    int segundo = 7;
    int *destino = &segundo;
    printf("Antes: %d y %d\n", primero, segundo);
    intercambiar(&primero, destino);
    printf("Después: %d y %d\n", primero, segundo);
    return 0;
}
