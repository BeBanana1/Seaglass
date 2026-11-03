package dev.seaglass.app

import android.os.Bundle
import androidx.activity.enableEdgeToEdge

class MainActivity : TauriActivity() {
  // Retour passe d'abord par l'historique de la WebView : l'UI y pose une entrée tant
  // qu'elle a une page ou un panneau à fermer (`src/lib/backButton.ts`). Sinon, Android
  // reçoit Retour et passe l'app en arrière-plan.
  override val handleBackNavigation: Boolean = true

  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
  }
}
