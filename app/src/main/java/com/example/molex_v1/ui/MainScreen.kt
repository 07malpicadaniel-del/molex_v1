package com.example.molex_v1.ui

import androidx.compose.foundation.layout.PaddingValues
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

    val navBackStackEntry by navController.currentBackStackEntryAsState()
    val currentRoute = navBackStackEntry?.destination?.route
    // Ocultar la barra inferior de navegación de forma exclusiva cuando estemos en RemoteScreen
    val showBottomBar = currentRoute != BottomNavItem.Remote.route

    Scaffold(
        bottomBar = {
            if (showBottomBar) {
                NavigationBar {
                    items.forEach { item ->
                        NavigationBarItem(
                            icon = { Icon(imageVector = item.icon, contentDescription = item.title) },
                            label = { Text(item.title) },
                            selected = currentRoute == item.route,
                            onClick = {
                                navController.navigate(item.route) {
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
        }
    ) { innerPadding ->
        NavHost(
            navController = navController,
            startDestination = BottomNavItem.Devices.route, // Por defecto inicia en la lista de dispositivos/sesiones
            modifier = Modifier.padding(if (showBottomBar) innerPadding else PaddingValues())
        ) {
            composable(BottomNavItem.Ssh.route) { SshScreen(viewModel) }
            composable(BottomNavItem.Metrics.route) { MetricsScreen(viewModel) }
            composable(BottomNavItem.Remote.route) { 
                RemoteScreen(
                    viewModel = viewModel,
                    onNavigateToDevices = {
                        navController.navigate(BottomNavItem.Devices.route) {
                            popUpTo(navController.graph.startDestinationId)
                            launchSingleTop = true
                        }
                    }
                ) 
            }
            composable(BottomNavItem.Settings.route) { SettingsScreen() }
            composable(BottomNavItem.Devices.route) { 
                DevicesScreen(
                    viewModel = viewModel, 
                    onDeviceSelected = {
                        navController.navigate(BottomNavItem.Remote.route) {
                            popUpTo(navController.graph.startDestinationId)
                            launchSingleTop = true
                        }
                    }
                ) 
            }
        }
    }
}
