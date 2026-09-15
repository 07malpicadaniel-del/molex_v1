package com.example.molex_v1.ui

import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Icon
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.currentBackStackEntryAsState
import androidx.navigation.compose.rememberNavController
import com.example.molex_v1.MolexViewModel
import com.example.molex_v1.ui.navigation.BottomNavItem
import com.example.molex_v1.ui.screens.DevicesScreen
import com.example.molex_v1.ui.screens.MetricsScreen
import com.example.molex_v1.ui.screens.SettingsScreen
import com.example.molex_v1.ui.screens.SshScreen

@Composable
fun MainScreen(viewModel: MolexViewModel) {
    val navController = rememberNavController()

    val items = listOf(
        BottomNavItem.Ssh,
        BottomNavItem.Metrics,
        BottomNavItem.Remote,
        BottomNavItem.Settings,
        BottomNavItem.Devices
    )

    Scaffold(
        bottomBar = {
            NavigationBar {
                val navBackStackEntry by navController.currentBackStackEntryAsState()
                val currentRoute = navBackStackEntry?.destination?.route

                items.forEach { item ->
                    NavigationBarItem(
                        icon = { Icon(imageVector = item.icon, contentDescription = item.title) },
                        label = { Text(item.title) },
                        selected = currentRoute == item.route,
                        onClick = {
                            navController.navigate(item.route) {
                                // Navegación limpia para no acumular vistas en la pila de atrás
                                popUpTo(navController.graph.startDestinationId) {
                                    saveState = true
                                }
                                launchSingleTop = true
                                restoreState = true
                            }
                        }
                    )
                }
            }
        }
    ) { innerPadding ->
        NavHost(
            navController = navController,
            startDestination = BottomNavItem.Remote.route, // Por defecto inicia en la vista del Control
            modifier = Modifier.padding(innerPadding)
        ) {
            composable(BottomNavItem.Ssh.route) { SshScreen() }
            composable(BottomNavItem.Metrics.route) { MetricsScreen() }
            composable(BottomNavItem.Remote.route) { RemoteScreen(viewModel) }
            composable(BottomNavItem.Settings.route) { SettingsScreen() }
            composable(BottomNavItem.Devices.route) { DevicesScreen() }
        }
    }
}