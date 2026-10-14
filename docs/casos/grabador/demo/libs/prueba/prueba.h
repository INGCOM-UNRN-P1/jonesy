/*
 * prueba.h: mini biblioteca de pruebas de la demo (imita la salida de p1_test).
 *
 *   TEST(nombre) { ASSERT_INT_EQ(esperado, obtenido); }
 *   int main(int argc, char *argv[]) { RUN_TEST(nombre); return RESUMEN(); }
 *
 * `./test_bin -k nombre` corre sólo esa prueba.
 */
#ifndef PRUEBA_H
#define PRUEBA_H

#include <stdio.h>
#include <string.h>

static const char *prueba_actual = "";
static const char *prueba_filtro = NULL;
static int prueba_fallo = 0;
static int prueba_aprobados = 0;
static int prueba_desaprobados = 0;

#define TEST(nombre) static void nombre(void)

#define PRUEBA_ARGS(argc, argv) \
    do { \
        if ((argc) > 2 && strcmp((argv)[1], "-k") == 0) { \
            prueba_filtro = (argv)[2]; \
        } \
    } while (0)

#define ASSERT_INT_EQ(esperado, obtenido) \
    do { \
        long e_ = (esperado); \
        long o_ = (obtenido); \
        if (e_ != o_) { \
            printf("  [FALLO] %s:%d: ASSERT_INT_EQ(%s, %s)\n", __FILE__, __LINE__, \
                   #esperado, #obtenido); \
            printf("    en test: %s\n    esperado: igual a %ld\n    obtenido: %ld\n", \
                   prueba_actual, e_, o_); \
            prueba_fallo = 1; \
        } \
    } while (0)

#define RUN_TEST(nombre) \
    do { \
        if (prueba_filtro == NULL || strcmp(prueba_filtro, #nombre) == 0) { \
            prueba_actual = #nombre; \
            prueba_fallo = 0; \
            nombre(); \
            printf("[CORRIENDO] %s ... %s\n", #nombre, prueba_fallo ? "FALLÓ" : "PASÓ"); \
            if (prueba_fallo) { \
                prueba_desaprobados++; \
            } else { \
                prueba_aprobados++; \
            } \
        } \
    } while (0)

#define RESUMEN() \
    (printf("Tests aprobados: %d\nTests desaprobados: %d\n", prueba_aprobados, \
            prueba_desaprobados), \
     prueba_desaprobados != 0)

#endif
