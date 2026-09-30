package com.example.molex_v1

import android.graphics.BitmapFactory
import android.util.Base64
import android.util.Log
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.example.molex_v1.data.db.DeviceDao
import com.example.molex_v1.data.db.DeviceEntity
import com.example.molex_v1.utils.stripAnsiCodes
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import uniffi.client.MolexInputClient
import uniffi.client.MolexVideoClient
import uniffi.client.ServerProfile
import uniffi.client.SystemMetrics

sealed class VideoState {
    object Idle : VideoState()
    object Loading : VideoState()
    data class Success(val frame: ImageBitmap) : VideoState()
    data class Error(val message: String) : VideoState()
}

class MolexViewModel(
    private val deviceDao: DeviceDao
) : ViewModel() {

    // Clientes nativos Rust. Los mantenemos nulos hasta conectarnos
    private var videoClient: MolexVideoClient? = null
    private var inputClient: MolexInputClient? = null

    // Jobs de corrutinas para poder cancelarlos al cambiar de servidor
    private var videoJob: Job? = null
    private var metricsJob: Job? = null

    private val _activeHost = MutableStateFlow<String?>(null)
    val activeHost = _activeHost.asStateFlow()

    private val _isConnected = MutableStateFlow(false)
    val isConnected = _isConnected.asStateFlow()

    private val _currentFrame = MutableStateFlow<ImageBitmap?>(null)
    val currentFrame = _currentFrame.asStateFlow()

    private val _videoState = MutableStateFlow<VideoState>(VideoState.Idle)
    val videoState = _videoState.asStateFlow()

    private val _systemMetrics = MutableStateFlow<SystemMetrics?>(null)
    val systemMetrics = _systemMetrics.asStateFlow()

    private val _terminalLogs = MutableStateFlow<List<String>>(emptyList())
    val terminalLogs = _terminalLogs.asStateFlow()

    // Lista y selección de monitores desde Rust FFI
    private val _availableMonitors = MutableStateFlow<List<String>>(emptyList())
    val availableMonitors = _availableMonitors.asStateFlow()

    private val _selectedMonitor = MutableStateFlow<String?>(null)
    val selectedMonitor = _selectedMonitor.asStateFlow()

    // Gestión de Dispositivos persistidos con Room
    val savedDevices: StateFlow<List<DeviceEntity>> = deviceDao.getAllDevices()
        .stateIn(
            scope = viewModelScope,
            started = SharingStarted.WhileSubscribed(5000),
            initialValue = emptyList()
        )

    fun addDevice(profile: ServerProfile) {
        viewModelScope.launch(Dispatchers.IO) {
            deviceDao.insertDevice(DeviceEntity.fromServerProfile(profile))
        }
    }

    fun deleteDevice(device: DeviceEntity) {
        viewModelScope.launch(Dispatchers.IO) {
            deviceDao.deleteDevice(device)
        }
    }

    fun selectMonitor(monitorName: String) {
        _selectedMonitor.value = monitorName
    }

    /**
     * Cierra la sesión activa destruyendo los clientes FFI y cancelando los Jobs.
     */
    fun disconnect() {
        videoJob?.cancel()
        metricsJob?.cancel()
        videoJob = null
        metricsJob = null

        videoClient?.destroy()
        inputClient?.destroy()
        videoClient = null
        inputClient = null

        _isConnected.value = false
        _activeHost.value = null
        _currentFrame.value = null
        _systemMetrics.value = null
        _availableMonitors.value = emptyList()
        _selectedMonitor.value = null
        _videoState.value = VideoState.Idle
    }

    /**
     * Inicia una conexión al servidor especificado.
     * Si ya hay una conexión activa, la destruye limpiamente para evitar fugas de memoria.
     */
    fun connectToServer(profile: ServerProfile) {
        val sanitizedProfile = ServerProfile(
            host = profile.host.trim(),
            port = profile.port,
            username = profile.username.trim(),
            password = profile.password?.trim()?.ifBlank { null }
        )

        disconnect()
        _activeHost.value = sanitizedProfile.host
        _videoState.value = VideoState.Loading

        viewModelScope.launch(Dispatchers.IO) {
            try {
                val newVideoClient = MolexVideoClient(sanitizedProfile)
                val newInputClient = MolexInputClient(sanitizedProfile.host)
                
                videoClient = newVideoClient
                inputClient = newInputClient
                
                _isConnected.value = true

                val monitors = newVideoClient.getMonitors()
                _availableMonitors.value = monitors
                _selectedMonitor.value = monitors.firstOrNull()

                startMetricsLoop()
            } catch (e: Exception) {
                e.printStackTrace()
                _isConnected.value = false
                _videoState.value = VideoState.Error(e.localizedMessage ?: "Conexión perdida")
            }
        }
    }

    // El procesamiento de video ha sido migrado a H264StreamDecoder.kt
    // y es gestionado directamente por RemoteScreen mediante SurfaceView

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
                            cpuLoad = rawMetrics.cpuLoad.stripAnsiCodes(),
                            gpuInfo = rawMetrics.gpuInfo.stripAnsiCodes(),
                            networkStatus = rawMetrics.networkStatus.stripAnsiCodes()
                        )
                    }
                } catch (e: Exception) {
                    if (e is kotlinx.coroutines.CancellationException) throw e
                    e.printStackTrace()
                    _systemMetrics.value = SystemMetrics(
                        osInfo = "ERROR DE CONEXIÓN",
                        ramUsage = e.localizedMessage ?: "Unknown",
                        cpuLoad = "Revisa Logcat",
                        gpuInfo = "Fallo FFI",
                        networkStatus = "Desconectado"
                    )
                }
                delay(1000L) 
            }
        }
    }

    fun sendCommandToSsh(cmd: String) {
        val trimmedCmd = cmd.trim()
        if (trimmedCmd.isBlank()) return

        if (videoClient == null) {
            _terminalLogs.value = _terminalLogs.value + "> $trimmedCmd" + "Error: Not connected to any device. Please go to 'Devices' tab."
            return
        }

        viewModelScope.launch(Dispatchers.IO) {
            _terminalLogs.value = _terminalLogs.value + "> $trimmedCmd"
            try {
                val response = videoClient?.executeCommand(trimmedCmd)
                val output = response?.stripAnsiCodes()
                if (output != null) {
                    _terminalLogs.value = _terminalLogs.value + output
                }
            } catch (e: Exception) {
                _terminalLogs.value = _terminalLogs.value + "Error: ${e.localizedMessage ?: "Command failed"}"
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
        disconnect()
    }
}
