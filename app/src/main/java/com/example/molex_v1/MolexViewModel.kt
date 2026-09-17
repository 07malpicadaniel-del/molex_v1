package com.example.molex_v1

import android.graphics.BitmapFactory
import android.util.Base64
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.example.molex_v1.utils.stripAnsiCodes
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import uniffi.client.MolexInputClient
import uniffi.client.MolexVideoClient
import uniffi.client.ServerProfile
import uniffi.client.SystemMetrics

class MolexViewModel : ViewModel() {

    // Clientes nativos Rust. Los mantenemos nulos hasta conectarnos
    private var videoClient: MolexVideoClient? = null
    private var inputClient: MolexInputClient? = null

    // Jobs de corrutinas para poder cancelarlos al cambiar de servidor
    private var videoJob: Job? = null
    private var metricsJob: Job? = null

    private val _currentFrame = MutableStateFlow<ImageBitmap?>(null)
    val currentFrame = _currentFrame.asStateFlow()

    private val _systemMetrics = MutableStateFlow<SystemMetrics?>(null)
    val systemMetrics = _systemMetrics.asStateFlow()

    // Gestión de Dispositivos (En memoria por ahora)
    private val _savedDevices = MutableStateFlow<List<ServerProfile>>(emptyList())
    val savedDevices = _savedDevices.asStateFlow()

    fun addDevice(profile: ServerProfile) {
        val currentList = _savedDevices.value.toMutableList()
        currentList.add(profile)
        _savedDevices.value = currentList
    }

    /**
     * Inicia una conexión al servidor especificado.
     * Si ya hay una conexión activa, la destruye limpiamente para evitar fugas de memoria.
     */
    fun connectToServer(profile: ServerProfile) {
        // Cancelar bucles activos
        videoJob?.cancel()
        metricsJob?.cancel()

        // Liberar sockets e instancias de Rust FFI previas
        videoClient?.destroy()
        inputClient?.destroy()

        // Reiniciar estado UI
        _currentFrame.value = null
        _systemMetrics.value = null

        // Instanciar nuevos clientes
        try {
            videoClient = MolexVideoClient(profile)
            inputClient = MolexInputClient(profile.host)

            // Arrancar bucles
            startVideoLoop()
            startMetricsLoop()
        } catch (e: Exception) {
            e.printStackTrace()
            // TODO: Notificar a la UI el error de conexión
        }
    }

    private fun startVideoLoop() {
        videoJob = viewModelScope.launch(Dispatchers.IO) {
            while (isActive) {
                try {
                    val frameData = videoClient?.getScreenFrame()
                    if (frameData != null && frameData != "SAME_FRAME") {
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
        metricsJob = viewModelScope.launch(Dispatchers.IO) {
            while (isActive) {
                try {
                    val rawMetrics = videoClient?.getSystemMetrics()
                    if (rawMetrics != null) {
                        // Limpiamos los códigos ANSI de la terminal antes de emitirlos a la UI
                        _systemMetrics.value = SystemMetrics(
                            osInfo = rawMetrics.osInfo.stripAnsiCodes(),
                            ramUsage = rawMetrics.ramUsage.stripAnsiCodes(),
                            cpuLoad = rawMetrics.cpuLoad.stripAnsiCodes()
                        )
                    }
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
                inputClient?.sendMouseMove(xPercent, yPercent)
            } catch (e: Exception) {
                e.printStackTrace()
            }
        }
    }

    fun onMouseClick(xPercent: Float, yPercent: Float, isRightClick: Boolean = false) {
        viewModelScope.launch(Dispatchers.IO) {
            try {
                inputClient?.sendMouseClick(xPercent, yPercent, isRightClick)
            } catch (e: Exception) {
                e.printStackTrace()
            }
        }
    }

    override fun onCleared() {
        super.onCleared()
        // Liberar recursos garantizados si el ViewModel muere
        videoJob?.cancel()
        metricsJob?.cancel()
        videoClient?.destroy()
        inputClient?.destroy()
    }
}
