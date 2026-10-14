#include "../prueba/prueba.h"
#include "vector.h"

TEST(suma_de_tres)
{
    int v[] = {1, 2, 3};
    ASSERT_INT_EQ(6, vector_suma(v, 3));
}

TEST(maximo_al_final)
{
    int v[] = {4, 1, 9};
    ASSERT_INT_EQ(9, vector_maximo(v, 3));
}

int main(int argc, char *argv[])
{
    PRUEBA_ARGS(argc, argv);
    RUN_TEST(suma_de_tres);
    RUN_TEST(maximo_al_final);
    return RESUMEN();
}
