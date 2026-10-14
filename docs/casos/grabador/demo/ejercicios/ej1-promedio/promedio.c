#include "promedio.h"
#include "vector.h"

double promedio(const int v[], size_t n)
{
    return (double)vector_suma(v, n) / n;
}
