package com.example.molex_v1.ui

import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.detectDragGestures
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.example.molex_v1.MolexViewModel

@Composable
fun RemoteScreen(viewModel: MolexViewModel) {
    val currentFrame by viewModel.currentFrame.collectAsStateWithLifecycle()

    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(MaterialTheme.colorScheme.background)
    ) {
        // Renderizado exclusivo del video y gestos táctiles
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
    }
}
