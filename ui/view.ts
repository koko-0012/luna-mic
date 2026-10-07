// Desktop layout only. Keep audio, settings writes and event bindings in main.ts.
import type { Config, Preset } from './types';
import { genericMicrophone as microphone } from './microphone-artwork';
import { esc, slider, toggle } from './controls';

export function renderView(config: Config, builtins: Preset[], advanced: boolean): string {
  const p = config.parameters;
  return /* HTML */ `<header>
      <div>
        <b>Luna Mic</b>
        <span class="version">0.7 / LOCAL AUDIO</span>
      </div>
      <button id="advanced" class="text-button" aria-expanded="${advanced}">
        Advanced ${advanced ? '−' : '+'}
      </button>
    </header>
    <div id="notice" class="notice" role="alert" hidden></div>
    <section class="device">
      <img id="device-artwork" src="${microphone}" alt="Generic microphone illustration" />
      <div>
        <span class="eyebrow">MIC DEVICE</span>
        <h1 id="device-name">Choose a microphone</h1>
        <p id="device-info">Windows recording device</p>
        <span id="active" class="status">● CONNECTING</span>
      </div>
    </section>
    <div class="device-select">
      <select id="input" aria-label="Microphone">
        <option value="">Detecting microphones…</option>
      </select>
      <button id="refresh" class="icon-button" aria-label="Refresh audio devices">↻</button>
    </div>
    <div class="default-actions">
      <button id="make-default" class="text-button" ${config.input === null ? 'disabled' : ''}>
        Set as Windows default mic
      </button>
      <button id="sound-settings" class="text-button">Windows Sound settings ↗</button>
    </div>
    <p class="hint">
      Sets the selected physical mic for Windows audio and voice calls. Luna Mic effects reach other
      apps only through a virtual cable.
    </p>
    <details class="quick-guide">
      <summary>What do the controls do?</summary>
      <dl>
        <dt>Input gain</dt>
        <dd>Turns your mic level up or down. Reduce it if CLIPPING appears.</dd>
        <dt>Bass / Warmth</dt>
        <dd>Add low-end weight or fullness. Too much can make speech muddy.</dd>
        <dt>Treble / Clarity</dt>
        <dd>Add brightness or make speech easier to hear. Too much can sound sharp.</dd>
        <dt>Noise gate</dt>
        <dd>
          Mutes quiet sounds between words. Move the cutoff above room noise and below your voice,
          or use Auto calibrate.
        </dd>
        <dt>Compressor</dt>
        <dd>Reduces loud voice peaks for a more even level.</dd>
        <dt>Limiter</dt>
        <dd>Catches peaks caused by effects. Keep it on for normal use.</dd>
        <dt>Hear myself</dt>
        <dd>
          Plays your mic in headphones. Original skips effects; With effects lets you compare.
        </dd>
        <dt>Output / Effects</dt>
        <dd>
          Output muted sends silence but keeps the meter running. Effects off sends your original
          voice.
        </dd>
      </dl>
    </details>
    <section class="meter-section">
      <div class="section-title">
        <h2>Input level</h2>
        <span id="level-db">−∞ dB</span>
        <span id="clip" hidden>CLIPPING</span>
      </div>
      <div class="meter">
        <div
          id="meter-fill"
          role="meter"
          aria-label="Average microphone level"
          aria-valuemin="-80"
          aria-valuemax="0"
          aria-valuenow="-80"
        ></div>
        <div id="gate-peak" aria-hidden="true"></div>
        <div
          id="threshold"
          role="slider"
          tabindex="0"
          aria-label="Noise gate cutoff; drag or use arrow keys"
          aria-valuemin="-80"
          aria-valuemax="-5"
          aria-valuenow="${p.gate_db}"
          title="Drag to change the quiet cutoff"
        >
          <span id="threshold-label">GATE ${p.gate_db} dB</span>
        </div>
      </div>
      <div class="meter-scale">
        <span>−80 · SILENT</span>
        <span>−40 · NOISE / VOICE</span>
        <span>0 · LOUD</span>
      </div>
      <div class="meter-footer">
        <span id="gate-state">GATE CLOSED</span>
        <button id="calibrate" class="text-button">Auto calibrate</button>
      </div>
      <p class="hint">
        Drag GATE to set the cutoff. The bar is average loudness; the thin peak marker shows short
        sounds that can open the gate. Hold/release keeps it open briefly afterward.
      </p>
      <div id="calibration" hidden role="status">
        <p id="calibration-text"></p>
        <div class="calibration-actions">
          <button id="apply-calibration" hidden>Apply suggested gate</button>
          <button id="cancel-calibration" class="text-button">Cancel</button>
        </div>
      </div>
    </section>
    <section>
      <div class="section-title">
        <h2>Your sound</h2>
        <select id="preset" aria-label="Sound preset">
          ${[...builtins, ...config.presets, { name: 'Custom' }].map((x) => `<option ${x.name === config.preset ? 'selected' : ''}>${esc(x.name)}</option>`).join('')}
        </select>
      </div>
      ${slider('gain', 'Input gain', p.gain_db, -24, 18, 0.5, ' dB')}
      ${slider('bass', 'Bass', p.eq[1], -12, 12, 0.5)}
      ${slider('warmth', 'Warmth', p.eq[2], -12, 12, 0.5)}
      ${slider('treble', 'Treble', p.eq[7], -12, 12, 0.5)}
      ${slider('clarity', 'Clarity', p.eq[6], -12, 12, 0.5)}
      <p class="hint">
        Shape your voice manually, or start from a preset. Deep adds bass; it does not lower pitch.
      </p>
    </section>
    <section>
      <div class="section-title">
        <h2>Cleanup</h2>
        <span id="reduction"> </span>
      </div>
      ${toggle('suppression', 'Noise removal <small>Local · one method at a time</small>', p.suppression_on)}
      <label class="field"
        >Removal method<select id="noise-method" aria-label="Noise-removal method">
          ${[
            ['rnnoise', 'RNNoise'],
            ['speex', 'SpeexDSP'],
            ['webrtc', 'WebRTC'],
            ['deepfilter', 'DeepFilterNet 3'],
          ]
            .map(
              ([id, name]) =>
                `<option value="${id}" ${config.noise_method === id ? 'selected' : ''}>${name}</option>`,
            )
            .join('')}
        </select></label
      >
      ${slider('suppression-strength', 'Removal', p.suppression_strength * 100, 0, 100, 5, '%')}
      <p class="hint" id="suppression-info">
        Reduces background noise and some keyboard clicks. Full removal adds about 20 ms. Lower it
        if your voice sounds unnatural.
      </p>
      ${toggle('gate', 'Noise gate <small>Mute quiet sounds</small>', p.gate_on)}
      ${slider('gate-db', 'Quiet cutoff', p.gate_db, -80, -5, 1, ' dB')}
      ${toggle('compressor', 'Compressor', p.compressor_on)}
      ${slider('compression', 'Compression', Math.round(((p.ratio - 1) / 11) * 100), 0, 100, 1, '%')}
      ${toggle('limiter', 'Limiter <small id="limiter-state">Peak protection</small>', p.limiter_on)}
    </section>
    <section>
      <div class="section-title">
        <h2>Monitor</h2>
        <span class="hint">Use headphones to avoid feedback</span>
      </div>
      <p id="monitor-warning" class="monitor-warning" role="status" hidden></p>
      ${toggle('monitor', 'Hear myself', config.monitor)}
      <div class="segmented">
        <button
          id="raw"
          class="${config.monitor_raw ? 'selected' : ''}"
          aria-pressed="${config.monitor_raw}"
        >
          Original
        </button>
        <button
          id="processed"
          class="${!config.monitor_raw ? 'selected' : ''}"
          aria-pressed="${!config.monitor_raw}"
        >
          With effects
        </button>
        <span id="output-db" class="hint">Output — dB</span>
      </div>
    </section>
    <section class="driver-setup">
      <div class="section-title">
        <h2>VB-CABLE setup</h2>
        <span id="virtual-status" class="hint">Checking virtual microphone…</span>
      </div>
      <p class="hint">
        Required to use Luna Mic as your microphone in Discord, OBS and games. VB-CABLE is
        donationware by VB-Audio. Donations are welcome.
      </p>
      <button id="install-driver" disabled>Install VB-CABLE</button>
      <p id="driver-install-status" class="hint" role="status">Checking driver package…</p>
      <p class="hint">Windows administrator permission is required. Restart after installing.</p>
    </section>
    <section>
      <div class="section-title"><h2>Output</h2></div>
      <select id="route-output" aria-label="Processed routing output">
        <option value="">Meter only · no routed audio</option>
      </select>
      <button id="use-virtual-mic" class="text-button" disabled>Route to VB-CABLE</button>
      <p id="virtual-help" class="hint">
        Use CABLE Input here and CABLE Output as your microphone in Discord / OBS.
      </p>
    </section>
    <div id="advanced-panel" ${advanced ? '' : 'hidden'}>
      <section>
        <h2>Advanced EQ</h2>
        ${toggle('eq-on', 'Equalizer', p.eq_on)}
        ${[60, 120, 250, 500, 1000, 2000, 4000, 8000].map((hz, i) => slider('eq-' + i, hz < 1000 ? hz + ' Hz' : hz / 1000 + ' kHz', p.eq[i], -12, 12, 0.5, ' dB')).join('')}
      </section>
      <section>
        <h2>Dynamics</h2>
        ${toggle('gain-on', 'Input gain module', p.gain_on)}
        ${slider('gate-attack', 'Gate attack', p.gate_attack, 1, 100, 1, ' ms')}
        ${slider('gate-hold', 'Gate hold', p.gate_hold, 0, 1000, 10, ' ms')}
        ${slider('gate-release', 'Gate release', p.gate_release, 10, 2000, 10, ' ms')}
        ${slider('comp-threshold', 'Compressor threshold', p.compressor_db, -60, 0, 1, ' dB')}
        ${slider('ratio', 'Compressor ratio', p.ratio, 1, 12, 0.1, ':1')}
        ${slider('comp-attack', 'Compressor attack', p.compressor_attack, 1, 100, 1, ' ms')}
        ${slider('comp-release', 'Compressor release', p.compressor_release, 10, 2000, 10, ' ms')}
        ${slider('makeup', 'Makeup gain', p.makeup_db, 0, 12, 0.5, ' dB')}
        ${slider('ceiling', 'Limiter ceiling', p.ceiling_db, -12, -0.1, 0.1, ' dB')}
        <p class="hint">
          Limiter uses instant peak protection without lookahead. Extreme gain may distort. Bypass
          skips all DSP; audio endpoints still clamp to their valid sample range.
        </p>
      </section>
      <section>
        <h2>Monitoring configuration</h2>
        <label class="field"
          >Headphones / monitoring output<select id="monitor-output"></select>
        </label>
        <label class="field">Microphone input channel<select id="channel"></select> </label>
        <p class="hint">
          Removed-audio monitoring is not available yet. Capture uses one selected channel, never a
          stereo mix.
        </p>
      </section>
      <section>
        <h2>Settings</h2>
        ${toggle('tray', 'Close to system tray', config.minimize_to_tray)}
        ${toggle('minimized', 'Start minimized', config.start_minimized)}
        ${toggle('autostart', 'Start with Windows', config.start_with_windows)}
        ${toggle('remember-device', 'Remember microphone', config.remember_device)}
        ${toggle('remember-sound', 'Remember sound settings', config.remember_sound)}
        <p class="hint">
          Startup points to the current executable. Enable it from your release copy, then keep that
          file in place.
        </p>
      </section>
      <section>
        <h2>Personal presets</h2>
        <div class="inline">
          <input
            id="preset-name"
            type="text"
            maxlength="60"
            placeholder="Preset name"
            aria-label="New preset name"
          />
          <button id="save-preset">Save</button>
        </div>
        <div class="inline">
          <button id="export-preset">Export current</button>
          <button id="delete-preset" class="text-button">Delete selected custom preset</button>
        </div>
        <label class="field"
          >Import preset JSON<textarea
            id="import-json"
            rows="3"
            placeholder='{"name":"My sound","parameters":{…}}'
          ></textarea>
        </label>
        <button id="import-preset">Import</button>
        <p class="hint">
          Preset files contain sound settings only. Imports are validated by the Rust backend.
        </p>
      </section>
      <section>
        <h2>Diagnostics</h2>
        <pre id="diagnostics">Waiting for audio engine…</pre>
        <p class="hint">
          Callback time is measured DSP / routing work, not end-to-end latency. Queue delay excludes
          Windows / device buffers.
        </p>
      </section>
    </div>
    <p id="stream-message" class="notice" role="status"></p>
    <footer>
      <button
        id="processing"
        class="power ${p.enabled ? 'on' : ''}"
        title="${p.enabled ? 'Mute audio sent to headphones and routed output' : 'Unmute audio sent to headphones and routed output'}"
        aria-pressed="${p.enabled}"
      >
        ${p.enabled ? '● Output on' : '○ Output muted'}
      </button>
      <button
        id="bypass"
        class="${p.bypass ? 'selected' : ''}"
        title="Compare your original voice with your enabled effects"
        aria-pressed="${!p.bypass}"
      >
        ${p.bypass ? 'Effects off · original' : 'Effects on'}
      </button>
      <span id="save-status">Saved locally</span>
      <p class="mode-help">
        ${!p.enabled ? 'No audio leaves Luna Mic. Your level meter stays live.' : p.bypass ? 'Your original mic audio is sent without effects.' : 'Audio uses your enabled effects. Click Effects on to compare the original.'}
      </p>
    </footer>`;
}
