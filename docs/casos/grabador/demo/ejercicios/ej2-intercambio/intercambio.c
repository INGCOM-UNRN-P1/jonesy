#include "intercambio.h"

void intercambiar(int *a, int *b)
{
    int auxiliar = *a;
    *a = *b;
    *b = auxiliar;
}
