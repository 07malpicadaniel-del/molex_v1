package com.example.molex_v1.ui

import android.app.Activity
import android.content.pm.ActivityInfo
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.slideOutHorizontally
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
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.example.molex_v1.MolexViewModel
import com.example.molex_v1.VideoState
import com.example.molex_v1.utils.H264StreamDecoder
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch

@Composable
fun RemoteScreen(
    viewModel: MolexViewModel,
    onNavigateToDevices: () -> Unit = {}
) {
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
            val host by viewModel.activeHost.collectAsStateWithLifecycle()
            val scope = rememberCoroutineScope()
            var activeDecoder by remember { mutableStateOf<H264StreamDecoder?>(null) }

            AndroidView(
                factory = { ctx ->
                    android.view.SurfaceView(ctx).apply {
                        // 1. Evita que el fondo negro de Compose tape la superficie del video
                        setZOrderMediaOverlay(true)

                        // 2. Fija las dimensiones nativas exactas del monitor ultrawide
                        holder.setFixedSize(2560, 1080)

                        holder.addCallback(object : android.view.SurfaceHolder.Callback {
                            override fun surfaceCreated(holder: android.view.SurfaceHolder) {
                                val targetHost = host ?: viewModel.activeHost.value
                                if (!targetHost.isNullOrBlank() && holder.surface.isValid) {
                                    val decoder = H264StreamDecoder(holder.surface)
                                    activeDecoder = decoder
                                    scope.launch(Dispatchers.IO) {
                                        decoder.start(targetHost, 9091)
                                    }
                                }
                            }

                            override fun surfaceChanged(
                                holder: android.view.SurfaceHolder,
                                format: Int,
                                width: Int,
                                height: Int
                            ) {}

                            override fun surfaceDestroyed(holder: android.view.SurfaceHolder) {
                                activeDecoder?.stop()
                                activeDecoder = null
                            }
                        })
                    }
                },
                modifier = Modifier
                    .fillMaxSize()
                    .pointerInput(Unit) {
                        detectDragGestures { change, _ ->
                            val mapped = calculateMappedCoordinates(
                                touchX = change.position.x,
                                touchY = change.position.y,
                                containerWidth = size.width.toFloat(),
                                containerHeight = size.height.toFloat(),
                                imageWidth = 2560f,
                                imageHeight = 1080f
                            )
                            if (mapped != null) {
                                change.consume()
                                viewModel.onMouseMove(mapped.first, mapped.second)
                            }
                        }
                    }
                    .pointerInput(Unit) {
                        detectTapGestures(
                            onTap = { offset ->
                                val mapped = calculateMappedCoordinates(
                                    touchX = offset.x,
                                    touchY = offset.y,
                                    containerWidth = size.width.toFloat(),
                                    containerHeight = size.height.toFloat(),
                                    imageWidth = 2560f,
                                    imageHeight = 1080f
                                )
                                if (mapped != null) {
                                    viewModel.onMouseClick(mapped.first, mapped.second, isRightClick = false)
                                }
                            },
                            onLongPress = { offset ->
                                val mapped = calculateMappedCoordinates(
                                    touchX = offset.x,
                                    touchY = offset.y,
                                    containerWidth = size.width.toFloat(),
                                    containerHeight = size.height.toFloat(),
                                    imageWidth = 2560f,
                                    imageHeight = 1080f
                                )
                                if (mapped != null) {
                                    viewModel.onMouseClick(mapped.first, mapped.second, isRightClick = true)
                                }
                            }
                        )
                    }
            )

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

private fun calculateMappedCoordinates(
    touchX: Float,
    touchY: Float,
    containerWidth: Float,
    containerHeight: Float,
    imageWidth: Float,
    imageHeight: Float
): Pair<Float, Float>? {
    if (containerWidth == 0f || containerHeight == 0f || imageWidth == 0f || imageHeight == 0f) return null

    val containerRatio = containerWidth / containerHeight
    val imageRatio = imageWidth / imageHeight

    val visibleWidth: Float
    val visibleHeight: Float
    val offsetX: Float
    val offsetY: Float

    if (imageRatio > containerRatio) {
        visibleWidth = containerWidth
        visibleHeight = containerWidth / imageRatio
        offsetX = 0f
        offsetY = (containerHeight - visibleHeight) / 2f
    } else {
        visibleHeight = containerHeight
        visibleWidth = containerHeight * imageRatio
        offsetX = (containerWidth - visibleWidth) / 2f
        offsetY = 0f
    }

    if (touchX < offsetX || touchX > offsetX + visibleWidth ||
        touchY < offsetY || touchY > offsetY + visibleHeight
    ) {
        return null
    }

    val clampedX = ((touchX - offsetX) / visibleWidth).coerceIn(0f, 1f)
    val clampedY = ((touchY - offsetY) / visibleHeight).coerceIn(0f, 1f)

    return Pair(clampedX, clampedY)
}
