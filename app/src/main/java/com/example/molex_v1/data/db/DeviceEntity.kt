package com.example.molex_v1.data.db

import androidx.room.Entity
import androidx.room.PrimaryKey
import uniffi.client.ServerProfile

@Entity(tableName = "devices")
data class DeviceEntity(
    @PrimaryKey(autoGenerate = true)
    val id: Long = 0,
    val host: String,
    val port: Int = 22,
    val username: String,
    val password: String? = null
) {
    fun toServerProfile(): ServerProfile {
        return ServerProfile(
            host = host.trim(),
            port = port.toUShort(),
            username = username.trim(),
            password = password?.trim()?.ifBlank { null }
        )
    }

    companion object {
        fun fromServerProfile(profile: ServerProfile): DeviceEntity {
            return DeviceEntity(
                host = profile.host.trim(),
                port = profile.port.toInt(),
                username = profile.username.trim(),
                password = profile.password?.trim()?.ifBlank { null }
            )
        }
    }
}
