package com.example.molex_v1.ui

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.detectDragGestures
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.example.molex_v1.MolexViewModel
import uniffi.client.SystemMetrics

@Composable
fun RemoteScreen(viewModel: MolexViewModel) {
    val currentFrame by viewModel.currentFrame.collectAsStateWithLifecycle()
    val metrics by viewModel.systemMetrics.collectAsStateWithLifecycle()

    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(Color.Black)
    ) {
        Canvas(
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
        ) {
            currentFrame?.let { imageBitmap ->
                drawImage(
                    image = imageBitmap,
                    dstSize = IntSize(size.width.toInt(), size.height.toInt())
                )
            }
        }

        metrics?.let {
            MetricsOverlay(
                metrics = it,
                modifier = Modifier
                    .align(Alignment.TopEnd)
                    .padding(16.dp)
            )
        }
    }
}

@Composable
fun MetricsOverlay(metrics: SystemMetrics, modifier: Modifier = Modifier) {
    Box(
        modifier = modifier
            .clip(RoundedCornerShape(8.dp))
            .background(Color.Black.copy(alpha = 0.6f))
            .padding(12.dp)
    ) {
        Column {
            Text(
                text = "OS: ${metrics.osInfo}",
                color = Color.White,
                fontSize = 12.sp,
                fontWeight = FontWeight.Bold
            )
            Text(
                text = "CPU: ${metrics.cpuLoad}",
                color = Color.Green,
                fontSize = 12.sp,
                fontWeight = FontWeight.SemiBold
            )
            Text(
                text = "RAM: ${metrics.ramUsage}",
                color = Color.Yellow,
                fontSize = 12.sp,
                fontWeight = FontWeight.SemiBold
            )
        }
    }
}
