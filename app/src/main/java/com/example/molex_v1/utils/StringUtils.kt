package com.example.molex_v1.utils

/**
 * Elimina los códigos de escape ANSI (frecuentes en las salidas de terminal Linux).
 * Usa una expresión regular robusta que identifica los secuencias "\u001B[...m" y similares.
 */
fun String.stripAnsiCodes(): String {
    val ansiRegex = Regex("\u001B\\[[;\\d]*[a-zA-Z]")
    return this.replace(ansiRegex, "").trim()
}