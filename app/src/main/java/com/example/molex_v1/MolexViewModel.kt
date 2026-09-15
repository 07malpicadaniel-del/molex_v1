package com.example.molex_v1

import android.graphics.BitmapFactory
import android.util.Base64
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import uniffi.client.MolexInputClient
import uniffi.client.MolexVideoClient
import uniffi.client.SystemMetrics

class MolexViewModel(
    private val videoClient: MolexVideoClient,
    private val inputClient: MolexInputClient
) : ViewModel() {

    private val _currentFrame = MutableStateFlow<ImageBitmap?>(null)
    val currentFrame = _currentFrame.asStateFlow()

    private val _systemMetrics = MutableStateFlow<SystemMetrics?>(null)
    val systemMetrics = _systemMetrics.asStateFlow()

    init {
        startVideoLoop()
        startMetricsLoop()
    }

    private fun startVideoLoop() {
        viewModelScope.launch(Dispatchers.IO) {
            while (isActive) {
                try {
                    val frameData = videoClient.getScreenFrame()
                    if (frameData != "SAME_FRAME") {
                        val bytes = Base64.decode(frameData, Base64.DEFAULT)
                        val bitmap = BitmapFactory.decodeByteArray(bytes, 0, bytes.size)
                        bitmap?.let { 
                            _currentFrame.value = it.asImageBitmap() 
                        }
                    }
                } catch (e: Exception) {
                    e.printStackTrace()
                }
            }
        }
    }

    private fun startMetricsLoop() {
        viewModelScope.launch(Dispatchers.IO) {
            while (isActive) {
                try {
                    _systemMetrics.value = videoClient.getSystemMetrics()
                } catch (e: Exception) {
                    e.printStackTrace()
                }
                delay(1000L) 
            }
        }
    }

    fun onMouseMove(xPercent: Float, yPercent: Float) {
        viewModelScope.launch(Dispatchers.IO) {
            try {
                inputClient.sendMouseMove(xPercent, yPercent)
            } catch (e: Exception) {
                e.printStackTrace()
            }
        }
    }

    fun onMouseClick(xPercent: Float, yPercent: Float, isRightClick: Boolean = false) {
        viewModelScope.launch(Dispatchers.IO) {
            try {
                inputClient.sendMouseClick(xPercent, yPercent, isRightClick)
            } catch (e: Exception) {
                e.printStackTrace()
            }
        }
    }
}
