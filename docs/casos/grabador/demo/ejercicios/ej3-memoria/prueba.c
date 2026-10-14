#include <stdlib.h>

#include "../../libs/prueba/prueba.h"
#include "copia.h"

TEST(copia_los_valores)
{
    int v[] = {1, 2, 3};
    int *c = copiar(v, 3);
    ASSERT_INT_EQ(3, c[2]);
    free(c);
}

int main(int argc, char *argv[])
{
    PRUEBA_ARGS(argc, argv);
    RUN_TEST(copia_los_valores);
    return RESUMEN();
}
