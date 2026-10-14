# Makefile general del TP: delega en la biblioteca y en cada ejercicio.
EJERCICIOS = ejercicios/ej1-promedio ejercicios/ej2-intercambio ejercicios/ej3-memoria

# Compila la biblioteca y todos los ejercicios
all: librerias
	@for e in $(EJERCICIOS); do $(MAKE) -s -C $$e || exit 1; done

# Compila las bibliotecas
librerias:
	@$(MAKE) -s -C libs/vector

# Corre las pruebas de la biblioteca y de los ejercicios
test: librerias
	@$(MAKE) -s -C libs/vector test
	@for e in $(EJERCICIOS); do $(MAKE) -s -C $$e test || exit 1; done

# Borra objetos y ejecutables
clean:
	@$(MAKE) -s -C libs/vector clean
	@for e in $(EJERCICIOS); do $(MAKE) -s -C $$e clean; done

.PHONY: all librerias test clean
