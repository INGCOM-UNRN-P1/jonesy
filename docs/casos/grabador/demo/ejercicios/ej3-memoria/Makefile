CC = gcc
CFLAGS = -std=c11 -g -Wall -Wextra
PROG_TARGET = copia
TEST_TARGET = test_bin

# Compila el programa y las pruebas
all: $(PROG_TARGET) $(TEST_TARGET)

$(PROG_TARGET): main.c copia.c copia.h
	$(CC) $(CFLAGS) -o $@ main.c copia.c

$(TEST_TARGET): prueba.c copia.c copia.h
	$(CC) $(CFLAGS) -o $@ prueba.c copia.c

# Ejecuta el programa
run: $(PROG_TARGET)
	./$(PROG_TARGET)

# Corre las pruebas del ejercicio
test: $(TEST_TARGET)
	./$(TEST_TARGET)

# Borra objetos y ejecutables
clean:
	rm -f *.o $(PROG_TARGET) $(TEST_TARGET)

.PHONY: all run test memcheck clean

# Revisa la memoria del programa con valgrind
memcheck: $(PROG_TARGET)
	valgrind --leak-check=full --error-exitcode=1 ./$(PROG_TARGET)
