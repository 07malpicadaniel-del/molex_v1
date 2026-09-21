package com.example.molex_v1

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.viewModels
import androidx.core.view.WindowCompat
import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import com.example.molex_v1.data.db.AppDatabase
import com.example.molex_v1.ui.MainScreen
import com.example.molex_v1.ui.theme.Molex_v1Theme

class MainActivity : ComponentActivity() {

    private val viewModel: MolexViewModel by viewModels {
        object : ViewModelProvider.Factory {
            @Suppress("UNCHECKED_CAST")
            override fun <T : ViewModel> create(modelClass: Class<T>): T {
                val database = AppDatabase.getDatabase(applicationContext)
                return MolexViewModel(database.deviceDao()) as T
            }
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        WindowCompat.setDecorFitsSystemWindows(window, false)

        setContent {
            Molex_v1Theme {
                MainScreen(viewModel = viewModel)
            }
        }
    }
}