"""Plugin de ejemplo para jonesy: copiar a .jonesy/plugins/todo.py.

al_guardar() devuelve diagnósticos que aparecen en el panel de Problemas y
como texto virtual; cada comando_<nombre>() aparece en la paleta (Ctrl+K)
como «todo: <nombre>».
"""


def al_guardar(archivo, texto):
    return [
        {"linea": n, "columna": linea.find("TODO") + 1,
         "mensaje": "queda un TODO por resolver", "severidad": "nota"}
        for n, linea in enumerate(texto.splitlines(), start=1)
        if "TODO" in linea
    ]


def comando_contar_lineas(ctx):
    lineas = ctx["texto"].splitlines()
    vacias = sum(1 for l in lineas if not l.strip())
    return f"**{len(lineas)}** líneas ({vacias} en blanco) en `{ctx['archivo']}`"
