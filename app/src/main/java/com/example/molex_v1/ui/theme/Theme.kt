package com.example.molex_v1.ui.theme

import android.app.Activity
import android.os.Build
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.dynamicDarkColorScheme
import androidx.compose.material3.dynamicLightColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.platform.LocalContext

// Forzamos la paleta oscura monocromática con acentos café
private val DarkColorScheme = darkColorScheme(
    primary = MolexBrownAccent,
    secondary = MolexBrownLight,
    tertiary = MolexBrownDark,
    background = MolexBackground,
    surface = MolexSurface,
    surfaceVariant = MolexSurfaceVariant,
    onPrimary = MolexBackground,
    onSecondary = MolexBackground,
    onTertiary = MolexTextPrimary,
    onBackground = MolexTextPrimary,
    onSurface = MolexTextPrimary,
    onSurfaceVariant = MolexTextSecondary
)

@Composable
fun Molex_v1Theme(
    // Siempre usaremos el tema oscuro por diseño, pero lo dejamos configurable por convención
    darkTheme: Boolean = true,
    // Dynamic color is available on Android 12+
    dynamicColor: Boolean = false,
    content: @Composable () -> Unit
) {
    val colorScheme = when {
        dynamicColor && Build.VERSION.SDK_INT >= Build.VERSION_CODES.S -> {
            val context = LocalContext.current
            if (darkTheme) dynamicDarkColorScheme(context) else dynamicLightColorScheme(context)
        }
        else -> DarkColorScheme
    }

    MaterialTheme(
        colorScheme = colorScheme,
        typography = Typography,
        content = content
    )
}
