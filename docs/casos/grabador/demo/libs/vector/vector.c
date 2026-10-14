#include "vector.h"

int vector_suma(const int v[], size_t n)
{
    int suma = 0;
    for (size_t i = 0; i < n; i++)
    {
        suma += v[i];
    }
    return suma;
}

int vector_maximo(const int v[], size_t n)
{
    int maximo = v[0];
    for (size_t i = 1; i < n; i++)
    {
        if (v[i] > maximo)
        {
            maximo = v[i];
        }
    }
    return maximo;
}
