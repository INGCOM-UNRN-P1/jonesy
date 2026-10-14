CC = gcc
CFLAGS = -std=c11 -g -Wall -Wextra
LIB = libvector.a
TEST_TARGET = test_bin

# Compila la biblioteca
all: $(LIB)

$(LIB): vector.o
	ar rcs $@ $^

vector.o: vector.c vector.h
	$(CC) $(CFLAGS) -c vector.c

$(TEST_TARGET): prueba.c $(LIB)
	$(CC) $(CFLAGS) -o $@ prueba.c $(LIB)

# Corre las pruebas de la biblioteca
test: $(TEST_TARGET)
	./$(TEST_TARGET)

# Borra objetos, la biblioteca y las pruebas
clean:
	rm -f *.o $(LIB) $(TEST_TARGET)

.PHONY: all test clean
