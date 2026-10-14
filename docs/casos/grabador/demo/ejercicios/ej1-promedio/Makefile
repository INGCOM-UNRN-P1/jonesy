CC = gcc
CFLAGS = -std=c11 -g -Wall -Wextra -I../../libs/vector
LIB_NAME = vector
LIBDIR = ../../libs/vector
PROG_TARGET = promedio
TEST_TARGET = test_bin

# Compila el programa y las pruebas
all: $(PROG_TARGET) $(TEST_TARGET)

$(LIBDIR)/libvector.a:
	$(MAKE) -s -C $(LIBDIR)

$(PROG_TARGET): main.c promedio.c promedio.h $(LIBDIR)/libvector.a
	$(CC) $(CFLAGS) -o $@ main.c promedio.c $(LIBDIR)/libvector.a

$(TEST_TARGET): prueba.c promedio.c promedio.h $(LIBDIR)/libvector.a
	$(CC) $(CFLAGS) -o $@ prueba.c promedio.c $(LIBDIR)/libvector.a

# Ejecuta el programa
run: $(PROG_TARGET)
	./$(PROG_TARGET)

# Corre las pruebas del ejercicio
test: $(TEST_TARGET)
	./$(TEST_TARGET)

# Revisa la memoria de las pruebas con valgrind
memcheck: $(TEST_TARGET)
	valgrind --leak-check=full --error-exitcode=1 ./$(TEST_TARGET)

# Borra objetos y ejecutables
clean:
	rm -f *.o $(PROG_TARGET) $(TEST_TARGET)

.PHONY: all run test memcheck clean
