package com.example.molex_v1.utils

/**
 * Elimina exclusivamente los códigos de escape ANSI reales de las salidas de terminal Linux.
 */
fun String.stripAnsiCodes(): String {
    return this.replace(Regex("\\x1B\\[[0-9;]*[a-zA-Z]"), "")
}
