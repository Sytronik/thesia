import numpy as np
import librosa


def _freq_expr(freqs):
    return [f'{f:.0f}' if f < 1000 else f'{f/1000:.2f}k' for f in freqs]


class Spectrogram:
    def __init__(self, wav, sr, win_ms, overlap, n_mel):
        if wav.ndim > 1:
            wav = wav.mean(axis=1)
        self._wav = wav
        self._sr = sr
        self._win_length = round(self._sr * win_ms / 1000)
        self._hop_length = round(self._win_length / overlap)
        self._n_fft = int(2**np.ceil(np.log2(self._win_length)))
        self._n_mel = n_mel
        self._floor_db = -120
        self._update_linear()

    def _amp_to_db(self, amp):
        return librosa.amplitude_to_db(amp, amin=10**(self._floor_db/20), top_db=-self._floor_db)

    def _pow_to_db(self, pow_):
        return librosa.power_to_db(pow_, amin=10**(self._floor_db/10), top_db=-self._floor_db)

    @property
    def wav(self):
        return self._wav

    @property
    def sr(self):
        return self._sr

    @property
    def n_fft(self):
        return self._n_fft

    @property
    def overlap(self):
        return round(self._win_length / self._hop_length)

    @overlap.setter
    def overlap(self, overlap):
        hop_length = round(self._win_length / overlap)
        if hop_length == self._hop_length:
            return
        self._hop_length = hop_length
        self._update_linear()

    @property
    def win_ms(self):
        return round(self._win_length * 1000 / self._sr)

    @win_ms.setter
    def win_ms(self, win_ms):
        win_length = round(self._sr * win_ms / 1000)
        if win_length == self._win_length:
            return
        self._win_length = win_length
        self._n_fft = int(2**np.ceil(np.log2(win_length)))
        self._update_linear()

    @property
    def n_mel(self):
        return self._n_mel

    @n_mel.setter
    def n_mel(self, n_mel):
        if n_mel == self._n_mel:
            return
        self._n_mel = n_mel
        self._update_mel()

    def _update_linear(self):
        self._spec = np.abs(
            librosa.stft(
                self._wav,
                n_fft=self._n_fft, hop_length=self._hop_length, win_length=self._win_length,
            )
        ) * (1024 / self._win_length * self._sr / 48000)
        self.linear = self._amp_to_db(self._spec)
        self.t_axis = np.arange(self._spec.shape[1]) * self._hop_length / self._sr
        self.f_linear_axis = np.linspace(0, self._sr//2, num=self.n_fft//2+1)
        self.f_linear_axis_str = np.tile(
            np.array(_freq_expr(self.f_linear_axis))[:, np.newaxis],
            (1, len(self.t_axis))
        )
        self._update_mel()

    def _update_mel(self):
        _mel = librosa.filters.mel(self._sr, self._n_fft, self._n_mel) @ self._spec
        self.mel = self._amp_to_db(_mel)
        self.f_mel_axis = librosa.mel_frequencies(self._n_mel, fmax=self._sr//2)
        self.f_mel_axis_str = np.tile(
            np.array(_freq_expr(self.f_mel_axis))[:, np.newaxis],
            (1, len(self.t_axis))
        )
