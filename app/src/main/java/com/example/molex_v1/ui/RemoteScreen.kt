package com.example.molex_v1.ui

import android.app.Activity
import android.content.pm.ActivityInfo
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.slideOutHorizontally
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.detectDragGestures
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.*
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.ArrowDropDown
import androidx.compose.material.icons.filled.Menu
import androidx.compose.material.icons.filled.PowerSettingsNew
import androidx.compose.material.icons.filled.Tv
import androidx.compose.material.icons.filled.Warning
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.example.molex_v1.MolexViewModel
import com.example.molex_v1.VideoState

@Composable
fun RemoteScreen(
    viewModel: MolexViewModel,
    onNavigateToDevices: () -> Unit = {}
) {
    val currentFrame by viewModel.currentFrame.collectAsStateWithLifecycle()
    val videoState by viewModel.videoState.collectAsStateWithLifecycle()
    val availableMonitors by viewModel.availableMonitors.collectAsStateWithLifecycle()
    val selectedMonitor by viewModel.selectedMonitor.collectAsStateWithLifecycle()

    var showOverlayControls by remember { mutableStateOf(false) }
    var monitorMenuExpanded by remember { mutableStateOf(false) }

    // Lock screen orientation to Landscape while on RemoteScreen
    val context = LocalContext.current
    DisposableEffect(Unit) {
        val activity = context as? Activity
        val originalOrientation = activity?.requestedOrientation
        activity?.requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_LANDSCAPE
        onDispose {
            activity?.requestedOrientation = originalOrientation ?: ActivityInfo.SCREEN_ORIENTATION_UNSPECIFIED
        }
    }

    if (videoState is VideoState.Error) {
        val errorMessage = (videoState as VideoState.Error).message
        Box(
            modifier = Modifier
                .fillMaxSize()
                .background(Color(0xFF2B0000))
                .padding(24.dp),
            contentAlignment = Alignment.Center
        ) {
            Column(
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.Center
            ) {
                Icon(
                    imageVector = Icons.Default.Warning,
                    contentDescription = "Error de Conexión",
                    tint = Color(0xFFFF5252),
                    modifier = Modifier.size(64.dp)
                )
                Spacer(modifier = Modifier.height(16.dp))
                Text(
                    text = errorMessage,
                    style = MaterialTheme.typography.headlineSmall,
                    color = Color.White,
                    fontWeight = FontWeight.Bold,
                    textAlign = TextAlign.Center
                )
                Spacer(modifier = Modifier.height(24.dp))
                Button(
                    onClick = {
                        viewModel.disconnect()
                        onNavigateToDevices()
                    },
                    colors = ButtonDefaults.buttonColors(
                        containerColor = MaterialTheme.colorScheme.primary,
                        contentColor = MaterialTheme.colorScheme.onPrimary
                    )
                ) {
                    Icon(
                        imageVector = Icons.AutoMirrored.Filled.ArrowBack,
                        contentDescription = null,
                        modifier = Modifier.size(18.dp)
                    )
                    Spacer(modifier = Modifier.width(8.dp))
                    Text("Volver a Dispositivos")
                }
            }
        }
    } else {
        Box(
            modifier = Modifier
                .fillMaxSize()
                .background(Color.Black)
        ) {
            // Video rendering and touch gesture capture
            currentFrame?.let { imageBitmap ->
                Image(
                    bitmap = imageBitmap,
                    contentDescription = "Remote Linux Desktop",
                    contentScale = ContentScale.FillBounds,
                    modifier = Modifier
                        .fillMaxSize()
                        .pointerInput(Unit) {
                            detectDragGestures { change, _ ->
                                change.consume()
                                val xPercent = (change.position.x / size.width).coerceIn(0f, 1f)
                                val yPercent = (change.position.y / size.height).coerceIn(0f, 1f)
                                viewModel.onMouseMove(xPercent, yPercent)
                            }
                        }
                        .pointerInput(Unit) {
                            detectTapGestures(
                                onTap = { offset ->
                                    val xPercent = (offset.x / size.width).coerceIn(0f, 1f)
                                    val yPercent = (offset.y / size.height).coerceIn(0f, 1f)
                                    viewModel.onMouseClick(xPercent, yPercent, isRightClick = false)
                                },
                                onLongPress = { offset ->
                                    val xPercent = (offset.x / size.width).coerceIn(0f, 1f)
                                    val yPercent = (offset.y / size.height).coerceIn(0f, 1f)
                                    viewModel.onMouseClick(xPercent, yPercent, isRightClick = true)
                                }
                            )
                        }
                )
            } ?: run {
                Box(modifier = Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                    Column(horizontalAlignment = Alignment.CenterHorizontally) {
                        CircularProgressIndicator(
                            color = MaterialTheme.colorScheme.primary
                        )
                        Spacer(modifier = Modifier.height(16.dp))
                        Text(
                            text = "Connecting to Wayland environment...",
                            color = MaterialTheme.colorScheme.onBackground
                        )
                    }
                }
            }

            // Monitor Selector Dropdown Menu (Top Center Overlay)
            Box(
                modifier = Modifier
                    .align(Alignment.TopCenter)
                    .padding(top = 12.dp)
            ) {
                Surface(
                    onClick = { monitorMenuExpanded = !monitorMenuExpanded },
                    color = Color.Black.copy(alpha = 0.6f),
                    contentColor = Color.White,
                    shape = MaterialTheme.shapes.small
                ) {
                    Row(
                        modifier = Modifier.padding(horizontal = 12.dp, vertical = 6.dp),
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        Icon(
                            imageVector = Icons.Default.Tv,
                            contentDescription = null,
                            modifier = Modifier.size(16.dp),
                            tint = MaterialTheme.colorScheme.primary
                        )
                        Spacer(modifier = Modifier.width(6.dp))
                        Text(
                            text = selectedMonitor ?: "Select Monitor",
                            style = MaterialTheme.typography.bodyMedium,
                            fontWeight = FontWeight.Medium
                        )
                        Spacer(modifier = Modifier.width(4.dp))
                        Icon(
                            imageVector = Icons.Default.ArrowDropDown,
                            contentDescription = "Select Monitor",
                            modifier = Modifier.size(18.dp)
                        )
                    }
                }

                DropdownMenu(
                    expanded = monitorMenuExpanded,
                    onDismissRequest = { monitorMenuExpanded = false },
                    modifier = Modifier.background(MaterialTheme.colorScheme.surfaceVariant)
                ) {
                    if (availableMonitors.isEmpty()) {
                        DropdownMenuItem(
                            text = { Text(text = "No monitors found") },
                            onClick = { monitorMenuExpanded = false }
                        )
                    } else {
                        availableMonitors.forEach { monitor ->
                            DropdownMenuItem(
                                text = {
                                    Text(
                                        text = monitor,
                                        fontWeight = if (monitor == selectedMonitor) FontWeight.Bold else FontWeight.Normal
                                    )
                                },
                                onClick = {
                                    viewModel.selectMonitor(monitor)
                                    monitorMenuExpanded = false
                                }
                            )
                        }
                    }
                }
            }

            // Floating menu button overlay (top-left subtle control)
            SmallFloatingActionButton(
                onClick = { showOverlayControls = !showOverlayControls },
                containerColor = Color.Black.copy(alpha = 0.5f),
                contentColor = Color.White,
                modifier = Modifier
                    .align(Alignment.TopStart)
                    .padding(12.dp)
            ) {
                Icon(Icons.Default.Menu, contentDescription = "Toggle Control Menu")
            }

            // Overlay Navigation Rail / Sidebar Controls
            AnimatedVisibility(
                visible = showOverlayControls,
                enter = slideInHorizontally(initialOffsetX = { -it }),
                exit = slideOutHorizontally(targetOffsetX = { -it }),
                modifier = Modifier.align(Alignment.CenterStart)
            ) {
                Surface(
                    color = Color.Black.copy(alpha = 0.85f),
                    contentColor = Color.White,
                    modifier = Modifier
                        .fillMaxHeight()
                        .width(190.dp)
                        .padding(16.dp)
                ) {
                    Column(
                        verticalArrangement = Arrangement.spacedBy(16.dp),
                        horizontalAlignment = Alignment.Start
                    ) {
                        Text(
                            text = "Molex Remote",
                            style = MaterialTheme.typography.titleMedium,
                            color = MaterialTheme.colorScheme.primary
                        )
                        HorizontalDivider(color = Color.DarkGray)

                        Button(
                            onClick = { onNavigateToDevices() },
                            colors = ButtonDefaults.buttonColors(
                                containerColor = MaterialTheme.colorScheme.primary
                            ),
                            modifier = Modifier.fillMaxWidth()
                        ) {
                            Icon(
                                imageVector = Icons.AutoMirrored.Filled.ArrowBack,
                                contentDescription = null,
                                modifier = Modifier.size(18.dp)
                            )
                            Spacer(modifier = Modifier.width(8.dp))
                            Text("Devices")
                        }

                        Button(
                            onClick = {
                                viewModel.disconnect()
                                onNavigateToDevices()
                            },
                            colors = ButtonDefaults.buttonColors(
                                containerColor = MaterialTheme.colorScheme.error,
                                contentColor = MaterialTheme.colorScheme.onError
                            ),
                            modifier = Modifier.fillMaxWidth()
                        ) {
                            Icon(
                                imageVector = Icons.Default.PowerSettingsNew,
                                contentDescription = null,
                                modifier = Modifier.size(18.dp)
                            )
                            Spacer(modifier = Modifier.width(8.dp))
                            Text("Disconnect")
                        }
                    }
                }
            }
        }
    }
}
