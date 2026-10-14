#include "../../libs/prueba/prueba.h"
#include "promedio.h"

TEST(promedio_exacto)
{
    int v[] = {4, 5, 6};
    ASSERT_INT_EQ(5, (int)promedio(v, 3));
}

TEST(promedio_con_decimales)
{
    int v[] = {1, 2};
    ASSERT_INT_EQ(150, (int)(promedio(v, 2) * 100));
}

int main(int argc, char *argv[])
{
    PRUEBA_ARGS(argc, argv);
    RUN_TEST(promedio_exacto);
    RUN_TEST(promedio_con_decimales);
    return RESUMEN();
}
