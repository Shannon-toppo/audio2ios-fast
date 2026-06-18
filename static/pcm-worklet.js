class PCMWorkletProcessor extends AudioWorkletProcessor {
  constructor() {
    super();
    this.channels = 2;
    // Ring capacity 400ms / preRoll 150ms at 48kHz. Sized generously because the
    // path is Wi-Fi to a phone, where 50-100ms delivery jitter is normal; a 60ms
    // cushion underran constantly. The headroom between preRoll (150ms) and
    // bufferSize (400ms) absorbs bursts without overflowing.
    this.bufferSize = 19200;
    this.left = new Float32Array(this.bufferSize);
    this.right = new Float32Array(this.bufferSize);
    this.writeIndex = 0;
    this.readIndex = 0;
    this.bufferedFrames = 0;
    this.primed = false;
    this.preRoll = 7200;
    this.underruns = 0;
    this.overflows = 0;
    this.framesSinceReport = 0;

    this.port.onmessage = (e) => {
      const data = e.data.audio;
      const frames = data.length / this.channels;
      for (let i = 0; i < frames; i++) {
        this.left[this.writeIndex] = data[i * 2];
        this.right[this.writeIndex] = data[i * 2 + 1];
        this.writeIndex = (this.writeIndex + 1) % this.bufferSize;
      }
      this.bufferedFrames += frames;

      // Overflow: the producer (Windows capture clock) has run ahead of the
      // consumer (iOS playback clock), or a network burst arrived. writeIndex
      // has now passed readIndex and overwritten the oldest unread frames.
      // Advance readIndex past the dropped frames so the ring-buffer invariant
      //   (writeIndex - readIndex) mod bufferSize === bufferedFrames
      // is preserved. Without this, readIndex keeps reading across the
      // writeIndex seam, mixing fresh and stale samples — a glitch that recurs
      // every buffer wrap and never self-corrects, so quality degrades the
      // longer playback runs. Dropping only the surplus keeps the skip minimal:
      // under steady clock drift this is ~2-3 samples/sec (inaudible).
      if (this.bufferedFrames > this.bufferSize) {
        const overflow = this.bufferedFrames - this.bufferSize;
        this.readIndex = (this.readIndex + overflow) % this.bufferSize;
        this.bufferedFrames = this.bufferSize;
        this.overflows++;
      }
    };
  }

  process(inputs, outputs) {
    const outL = outputs[0][0];
    const outR = outputs[0][1];
    const needed = outL.length;

    if (!this.primed) {
      if (this.bufferedFrames >= this.preRoll) {
        this.primed = true;
      } else {
        outL.fill(0);
        if (outR) outR.fill(0);
        return true;
      }
    }

    if (this.bufferedFrames >= needed) {
      for (let i = 0; i < needed; i++) {
        outL[i] = this.left[this.readIndex];
        if (outR) outR[i] = this.right[this.readIndex];
        this.readIndex = (this.readIndex + 1) % this.bufferSize;
      }
      this.bufferedFrames -= needed;
    } else {
      // Underrun: consumer outran producer. Emit silence and drop back to the
      // unprimed state so we rebuild the full preRoll cushion before resuming,
      // instead of skating along the empty edge and clicking on every block.
      outL.fill(0);
      if (outR) outR.fill(0);
      this.underruns++;
      this.primed = false;
    }

    this.framesSinceReport += needed;
    if (this.framesSinceReport >= sampleRate) {
      this.port.postMessage({
        bufferedFrames: this.bufferedFrames,
        underruns: this.underruns,
        overflows: this.overflows,
      });
      this.framesSinceReport = 0;
      this.underruns = 0;
      this.overflows = 0;
    }

    return true;
  }
}

registerProcessor("pcm-worklet", PCMWorkletProcessor);
