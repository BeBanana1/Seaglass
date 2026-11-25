//! WASAPI exclusive mode (#245): the engine opens the device for itself alone.
//! Normalization is suspended (no filter set); the software volume stays available,
//! and the signal path then says the signal is no longer bit-perfect.

use super::actor::Player;
use crate::engine::Op;
use crate::i18n::{tr, trf};

/// Can the engine open an output in exclusive mode? mpv on Windows only: Media3
/// has no usable exclusive mode, and on Linux (#17) neither PipeWire nor PulseAudio
/// offers it through mpv (direct ALSA on `hw:` is still to be studied).
pub fn supported() -> bool {
    cfg!(all(desktop, windows))
}

impl Player {
    /// Setting changed: applied live (mpv reopens the output, a tiny gap is normal),
    /// then normalization is removed from or restored on the current track.
    pub(super) async fn set_exclusive(&mut self, on: bool) {
        let on = on && supported();
        if on == self.state.exclusive {
            return;
        }
        tracing::info!("exclusive mode: {on}");
        self.state.exclusive = on;
        self.note_device_profile();
        self.engine_op(Op::Exclusive(on)).await;
        let before = self.normalizer.effective();
        self.normalizer.bypass = on;
        self.on_normalization_changed(before).await;
    }

    /// The output does not open in exclusive mode (device already taken by another app, or
    /// format refused): playback stops, paused on the track, with a clear message. No
    /// silent switch to shared mode: Play tries again at the same position.
    pub(super) async fn on_exclusive_failed(&mut self) {
        tracing::warn!("cannot open the output in exclusive mode");
        self.engine_op(Op::Stop).await;
        self.log_close();
        self.loaded = false;
        self.next_loaded = None;
        self.preloading = false;
        self.preload_req += 1;
        self.state.paused = true;
        let message = match self.output.describe().name {
            Some(name) => trf!(
                "Impossible d'ouvrir « {name} » en mode exclusif : une autre application l'utilise peut-être",
                "Cannot open “{name}” in exclusive mode: another application may be using it"
            ),
            None => tr(
                "Impossible d'ouvrir la sortie audio en mode exclusif : une autre application l'utilise peut-être",
                "Cannot open the audio output in exclusive mode: another application may be using it",
            )
            .into(),
        };
        self.toast("error", message);
    }
}
