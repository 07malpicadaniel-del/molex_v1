package com.example.molex_v1.utils

import android.media.MediaCodec
import android.media.MediaFormat
import android.util.Log
import android.view.Surface
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.io.InputStream
import java.net.Socket

class H264StreamDecoder(private val surface: Surface) {

    private var socket: Socket? = null
    private var decoder: MediaCodec? = null
    private var drainThread: Thread? = null

    @Volatile
    var isRunning = false
        private set

    suspend fun start(host: String, port: Int = 9091) {
        withContext(Dispatchers.IO) {
            if (!surface.isValid) {
                Log.e("H264StreamDecoder", "Superficie no válida al iniciar")
                return@withContext
            }
            isRunning = true
            try {
                val format = MediaFormat.createVideoFormat(MediaFormat.MIMETYPE_VIDEO_AVC, 2560, 1080).apply {
                    setInteger(MediaFormat.KEY_MAX_INPUT_SIZE, 1024 * 1024)
                }

                val codec = MediaCodec.createDecoderByType(MediaFormat.MIMETYPE_VIDEO_AVC)
                codec.configure(format, surface, null, 0)
                codec.start()
                decoder = codec

                // Hilo de drenado: Solo renderiza a pantalla si el buffer contiene datos reales (size > 0)
                drainThread = Thread {
                    val bufferInfo = MediaCodec.BufferInfo()
                    while (isRunning) {
                        val currentDecoder = decoder ?: break
                        try {
                            var outputBufferIndex = currentDecoder.dequeueOutputBuffer(bufferInfo, 10000)
                            while (outputBufferIndex >= 0) {
                                val doRender = bufferInfo.size > 0
                                currentDecoder.releaseOutputBuffer(outputBufferIndex, doRender)
                                outputBufferIndex = currentDecoder.dequeueOutputBuffer(bufferInfo, 0)
                            }
                        } catch (e: Exception) {
                            if (!isRunning) break
                            Log.e("H264StreamDecoder", "Error en drain thread: ${e.message}")
                        }
                    }
                }.apply {
                    name = "H264-DrainThread"
                    priority = Thread.MAX_PRIORITY
                    start()
                }

                socket = Socket(host, port).apply { tcpNoDelay = true }
                val inputStream = socket?.getInputStream() ?: return@withContext

                processStream(inputStream)
            } catch (e: Exception) {
                Log.e("H264StreamDecoder", "Fallo al iniciar decodificador: ${e.message}", e)
            } finally {
                stop()
            }
        }
    }

    private fun processStream(inputStream: InputStream) {
        val buffer = ByteArray(2 * 1024 * 1024) // 2MB
        var bufferLength = 0
        var hasSps = false
        var hasPps = false
        var isWaitingForKeyframe = true

        while (isRunning) {
            val bytesRead = inputStream.read(buffer, bufferLength, buffer.size - bufferLength)
            if (bytesRead == -1) break
            bufferLength += bytesRead

            var offset = 0
            while (offset < bufferLength - 3) {
                val is4ByteStart = (offset + 3 < bufferLength) &&
                        buffer[offset] == 0.toByte() &&
                        buffer[offset + 1] == 0.toByte() &&
                        buffer[offset + 2] == 0.toByte() &&
                        buffer[offset + 3] == 1.toByte()

                val is3ByteStart = !is4ByteStart &&
                        buffer[offset] == 0.toByte() &&
                        buffer[offset + 1] == 0.toByte() &&
                        buffer[offset + 2] == 1.toByte()

                if (is4ByteStart || is3ByteStart) {
                    val startCodeSize = if (is4ByteStart) 4 else 3

                    var nextStartCodeOffset = -1
                    for (i in offset + startCodeSize until bufferLength - 2) {
                        val isNext4Byte = (i + 3 < bufferLength) &&
                                buffer[i] == 0.toByte() &&
                                buffer[i + 1] == 0.toByte() &&
                                buffer[i + 2] == 0.toByte() &&
                                buffer[i + 3] == 1.toByte()

                        val isNext3Byte = !isNext4Byte &&
                                buffer[i] == 0.toByte() &&
                                buffer[i + 1] == 0.toByte() &&
                                buffer[i + 2] == 1.toByte()

                        if (isNext4Byte || isNext3Byte) {
                            nextStartCodeOffset = i
                            break
                        }
                    }

                    if (nextStartCodeOffset != -1) {
                        val nalLength = nextStartCodeOffset - offset
                        val nalData = ByteArray(nalLength)
                        System.arraycopy(buffer, offset, nalData, 0, nalLength)

                        val nalType = nalData[startCodeSize].toInt() and 0x1F

                        // Protocolo de seguridad: No alimentar P-Frames sin SPS/PPS ni primer Keyframe
                        if (nalType == 7) {
                            hasSps = true
                            feedDecoder(nalData, isConfig = true)
                        } else if (nalType == 8) {
                            hasPps = true
                            feedDecoder(nalData, isConfig = true)
                        } else if (hasSps && hasPps) {
                            if (isWaitingForKeyframe) {
                                if (nalType == 5) { // IDR Keyframe inicial
                                    isWaitingForKeyframe = false
                                    feedDecoder(nalData, isConfig = false)
                                }
                            } else {
                                feedDecoder(nalData, isConfig = false)
                            }
                        }

                        offset = nextStartCodeOffset
                    } else {
                        break
                    }
                } else {
                    offset++
                }
            }

            if (offset > 0) {
                val remaining = bufferLength - offset
                System.arraycopy(buffer, offset, buffer, 0, remaining)
                bufferLength = remaining
            } else if (bufferLength == buffer.size) {
                // Evita interbloqueos si se llena el buffer sin delimitar
                bufferLength = 0
            }
        }
    }

    private fun feedDecoder(data: ByteArray, isConfig: Boolean) {
        val decoder = this.decoder ?: return
        var retries = 3
        while (retries > 0 && isRunning) {
            try {
                val inputBufferIndex = decoder.dequeueInputBuffer(10000)
                if (inputBufferIndex >= 0) {
                    val inputBuffer = decoder.getInputBuffer(inputBufferIndex)
                    if (inputBuffer != null) {
                        inputBuffer.clear()
                        inputBuffer.put(data)
                        val flags = if (isConfig) MediaCodec.BUFFER_FLAG_CODEC_CONFIG else 0
                        val pts = if (isConfig) 0L else System.nanoTime() / 1000
                        decoder.queueInputBuffer(inputBufferIndex, 0, data.size, pts, flags)
                    }
                    break
                } else {
                    retries--
                    Thread.sleep(2)
                }
            } catch (e: Exception) {
                Log.e("H264StreamDecoder", "Error alimentando MediaCodec: ${e.message}", e)
                break
            }
        }
    }

    fun stop() {
        isRunning = false
        try { socket?.close() } catch (_: Exception) {}
        socket = null

        try { drainThread?.join(300) } catch (_: Exception) {}
        drainThread = null

        try {
            decoder?.stop()
            decoder?.release()
        } catch (_: Exception) {}
        decoder = null
    }
}
