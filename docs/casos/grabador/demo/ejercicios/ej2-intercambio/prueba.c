#include "../../libs/prueba/prueba.h"
#include "intercambio.h"

TEST(intercambia_dos_valores)
{
    int a = 1;
    int b = 2;
    intercambiar(&a, &b);
    ASSERT_INT_EQ(2, a);
    ASSERT_INT_EQ(1, b);
}

int main(int argc, char *argv[])
{
    PRUEBA_ARGS(argc, argv);
    RUN_TEST(intercambia_dos_valores);
    return RESUMEN();
}
