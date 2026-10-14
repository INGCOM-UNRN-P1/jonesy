#define _POSIX_C_SOURCE 200809L
#include <stdio.h>
#include <unistd.h>

#include "promedio.h"

#define MAXIMO 100

int main(void)
{
    int numeros[MAXIMO];
    int n = 0;
    int leidos = 0;
    if (isatty(STDIN_FILENO))
    {
        printf("Cantidad de números: ");
    }
    leidos = scanf("%d", &n);
    if (leidos != 1 || n <= 0 || n > MAXIMO)
    {
        printf("Cantidad inválida.\n");
        return 1;
    }
    if (isatty(STDIN_FILENO))
    {
        printf("Los %d números: ", n);
    }
    for (int i = 0; i < n; i++)
    {
        leidos = scanf("%d", &numeros[i]);
        if (leidos != 1)
        {
            printf("Número inválido.\n");
            return 1;
        }
    }
    printf("Promedio: %.2f\n", promedio(numeros, (size_t)n));
    return 0;
}
