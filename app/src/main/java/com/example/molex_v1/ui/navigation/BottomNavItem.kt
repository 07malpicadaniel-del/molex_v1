package com.example.molex_v1.ui.navigation

import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Analytics
import androidx.compose.material.icons.filled.DesktopMac
import androidx.compose.material.icons.filled.Dns
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material.icons.filled.Terminal
import androidx.compose.ui.graphics.vector.ImageVector

sealed class BottomNavItem(
    val route: String,
    val title: String,
    val icon: ImageVector
) {
    object Ssh : BottomNavItem("ssh", "SSH", Icons.Default.Terminal)
    object Metrics : BottomNavItem("metrics", "Metrics", Icons.Default.Analytics)
    object Remote : BottomNavItem("remote", "Screen", Icons.Default.DesktopMac)
    object Settings : BottomNavItem("settings", "Settings", Icons.Default.Settings)
    object Devices : BottomNavItem("devices", "Devices", Icons.Default.Dns)
}
