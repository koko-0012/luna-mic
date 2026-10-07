import { renderView } from './view';
import { number, levelPosition } from './controls';
import type { Config, Preset, Parameters, Device, Snapshot, Initial, NoiseMethod } from './types';
import { invoke, isTauri } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { genericMicrophone as microphone, microphoneArtwork } from './microphone-artwork';
import './style.css';

const app = document.querySelector<HTMLElement>('#app')!;
const $ = <T extends HTMLElement = HTMLElement>(id: string) => document.getElementById(id) as T;
let config: Config,
  builtins: Preset[] = [];
let current: Snapshot | undefined;
let saving: Promise<void> = Promise.resolve();
let timer: ReturnType<typeof setTimeout> | undefined;
let dirty = false;
let revision = 0;
let focused = true;
let advanced = false;
let smoothed = -90;
let clipUntil = 0;
let deviceSignature = '';
let driverPackage: { install_available: boolean; message: string } | undefined;
let driverInstalling = false;
async function refreshDriverPackage() {
  try {
    driverPackage = await invoke('driver_package_status');
  } catch {
    driverPackage = {
      install_available: false,
      message: 'Could not check the driver package. Reopen Luna Mic to retry.',
    };
  }
  if (current) update(current);
}
function message(text: string) {
  $('notice').textContent = text;
  $('notice').hidden = !text;
}
function commit(custom = true) {
  // Debounce slider writes and serialize saves. The revision prevents an older
  // completed save from marking newer, still-pending edits as saved.
  if (custom) {
    config.preset = 'Custom';
    $<HTMLSelectElement>('preset').value = 'Custom';
  }
  dirty = true;
  revision++;
  clearTimeout(timer);
  timer = setTimeout(() => {
    const pending = structuredClone(config);
    const savedRevision = revision;
    saving = saving.then(async () => {
      try {
        await invoke('set_config', { config: pending });
        if (savedRevision === revision) {
          dirty = false;
          $('save-status').textContent = 'Saved locally';
        }
      } catch (e) {
        message(String(e));
        $('save-status').textContent = 'Could not save';
      }
    });
  }, 180);
  $('save-status').textContent = 'Saving…';
}
// Rebuild only for structural changes; live meters update existing elements.
function render() {
  app.innerHTML = renderView(config, builtins, advanced);
  app.insertAdjacentHTML(
    'beforeend',
    `<dialog id="dependency-dialog" aria-labelledby="dependency-title">
    <h2 id="dependency-title">Install VB-CABLE to use Luna Mic</h2>
    <p>Luna Mic needs a virtual microphone to send your processed voice to Discord, OBS and games.</p>
    <p>This opens the original VB-Audio driver setup. Windows will ask for administrator permission.
    In the VB-Audio window, press <b>Install Driver</b>. Restart Windows afterward.</p>
    <p>VB-CABLE is donationware by VB-Audio. Donations are welcome.</p>
    <button class="text-button vb-website">Official website / donate ↗</button>
    <p id="dependency-message" role="status">Checking installer…</p>
    <div class="default-actions"><button id="dependency-install" disabled>Install VB-CABLE</button>
    <button id="dependency-refresh">Check again</button><button id="dependency-close">Close</button></div>
  </dialog>`,
  );
  bind();
  updateThreshold();
  deviceSignature = '';
  if (current) update(current);
}
function bind() {
  $('dependency-close').onclick = () => void invoke('close_setup');
  $('dependency-refresh').onclick = async () => {
    await invoke('refresh_devices');
    await refreshDriverPackage();
  };
  document.querySelectorAll<HTMLElement>('.vb-website').forEach((button) => {
    button.onclick = () => void invoke('vb_cable_website');
  });
  $('dependency-dialog').addEventListener('cancel', (event) => {
    event.preventDefault();
    void invoke('close_setup');
  });
  $('dependency-install').onclick = () => $('install-driver').click();
  $('install-driver').onclick = async () => {
    if (driverInstalling || !driverPackage?.install_available) return;
    driverInstalling = true;
    if (current) update(current);
    try {
      message(await invoke<string>('install_virtual_driver'));
      await invoke('refresh_devices');
    } catch (e) {
      message(String(e));
    } finally {
      driverInstalling = false;
      await refreshDriverPackage();
    }
  };
  $('use-virtual-mic').onclick = () => {
    const endpoint = current?.devices.outputs.find(
      (device) =>
        device.name.toLowerCase().includes('cable input') &&
        device.name.toLowerCase().includes('vb-audio'),
    );
    if (!endpoint) return;
    config.route_output = endpoint.id;
    commit(false);
    message('In Discord / OBS, choose CABLE Output as your input.');
  };
  $<HTMLSelectElement>('noise-method').onchange = () => {
    config.noise_method = $<HTMLSelectElement>('noise-method').value as NoiseMethod;
    commit(false);
  };
  // Pointer capture preserves a drag when the cursor leaves the marker's hit area.
  const threshold = $('threshold');
  function setGate(db: number) {
    config.parameters.gate_db = Math.round(Math.max(-80, Math.min(-5, db)));
    syncSlider('gate-db', config.parameters.gate_db, ' dB');
    updateThreshold();
    commit();
  }
  function dragGate(event: PointerEvent) {
    const bounds = threshold.parentElement!.getBoundingClientRect();
    setGate(((event.clientX - bounds.left) / bounds.width) * 80 - 80);
  }
  threshold.onpointerdown = (event) => {
    event.preventDefault();
    threshold.focus();
    threshold.setPointerCapture(event.pointerId);
    dragGate(event);
  };
  threshold.onpointermove = (event) => {
    if (threshold.hasPointerCapture(event.pointerId)) dragGate(event);
  };
  threshold.onpointerup = (event) => {
    if (threshold.hasPointerCapture(event.pointerId))
      threshold.releasePointerCapture(event.pointerId);
  };
  threshold.onkeydown = (event) => {
    const step = event.shiftKey ? 5 : 1;
    const value =
      event.key === 'Home'
        ? -80
        : event.key === 'End'
          ? -5
          : event.key === 'ArrowLeft' || event.key === 'ArrowDown'
            ? config.parameters.gate_db - step
            : event.key === 'ArrowRight' || event.key === 'ArrowUp'
              ? config.parameters.gate_db + step
              : undefined;
    if (value !== undefined) {
      event.preventDefault();
      setGate(value);
    }
  };
  $<HTMLInputElement>('suppression').onchange = () => {
    config.parameters.suppression_on = $<HTMLInputElement>('suppression').checked;
    if (config.parameters.suppression_on) {
      config.monitor_raw = false;
      config.parameters.bypass = false;
    }
    render();
    commit();
  };
  bindSlider('suppression-strength', (value) => {
    config.parameters.suppression_strength = value / 100;
  });
  $('advanced').onclick = () => {
    advanced = !advanced;
    $('advanced-panel').hidden = !advanced;
    $('advanced').textContent = 'Advanced ' + (advanced ? '−' : '+');
    $('advanced').setAttribute('aria-expanded', String(advanced));
  };
  $('refresh').onclick = () => void invoke('refresh_devices');
  $('sound-settings').onclick = async () => {
    try {
      await invoke('open_sound_settings');
    } catch (e) {
      message(String(e));
    }
  };
  $('make-default').onclick = async () => {
    const selected = config.input;
    if (!selected) return;
    const button = $<HTMLButtonElement>('make-default');
    button.disabled = true;
    try {
      message(await invoke<string>('set_default_microphone', { selected }));
    } catch (e) {
      message(String(e));
    } finally {
      button.disabled = config.input === null;
    }
  };
  $('input').onchange = () => {
    config.input = $<HTMLSelectElement>('input').value || null;
    config.input_channel = 0;
    cancelCalibration();
    commit(false);
  };
  const toggles: Record<string, keyof Parameters> = {
    gate: 'gate_on',
    compressor: 'compressor_on',
    limiter: 'limiter_on',
    'gain-on': 'gain_on',
    'eq-on': 'eq_on',
  };
  for (const [id, key] of Object.entries(toggles))
    $<HTMLInputElement>(id).onchange = () => {
      (config.parameters as unknown as Record<string, unknown>)[key] =
        $<HTMLInputElement>(id).checked;
      commit();
      updateThreshold();
    };
  const prefs: Record<string, keyof Config> = {
    monitor: 'monitor',
    tray: 'minimize_to_tray',
    minimized: 'start_minimized',
    autostart: 'start_with_windows',
    'remember-device': 'remember_device',
    'remember-sound': 'remember_sound',
  };
  for (const [id, key] of Object.entries(prefs))
    $<HTMLInputElement>(id).onchange = () => {
      (config as unknown as Record<string, unknown>)[key] = $<HTMLInputElement>(id).checked;
      commit(false);
    };
  for (const id of ['monitor-output', 'route-output'])
    $<HTMLSelectElement>(id).onchange = () => {
      config[id === 'monitor-output' ? 'monitor_output' : 'route_output'] =
        $<HTMLSelectElement>(id).value || null;
      commit(false);
    };
  $<HTMLSelectElement>('channel').onchange = () => {
    config.input_channel = Number($<HTMLSelectElement>('channel').value);
    commit(false);
  };
  const sliders: Record<string, keyof Parameters> = {
    gain: 'gain_db',
    'gate-db': 'gate_db',
    'gate-attack': 'gate_attack',
    'gate-hold': 'gate_hold',
    'gate-release': 'gate_release',
    'comp-threshold': 'compressor_db',
    ratio: 'ratio',
    'comp-attack': 'compressor_attack',
    'comp-release': 'compressor_release',
    makeup: 'makeup_db',
    ceiling: 'ceiling_db',
  };
  for (const [id, key] of Object.entries(sliders))
    bindSlider(id, (n) => {
      (config.parameters as unknown as Record<string, unknown>)[key] = n;
      updateThreshold();
    });
  for (const [id, i] of Object.entries({ bass: 1, warmth: 2, treble: 7, clarity: 6 }))
    bindSlider(id, (n) => {
      config.parameters.eq[i] = n;
      syncSlider('eq-' + i, n, ' dB');
    });
  for (let i = 0; i < 8; i++)
    bindSlider('eq-' + i, (n) => {
      config.parameters.eq[i] = n;
      for (const [id, index] of Object.entries({ bass: 1, warmth: 2, treble: 7, clarity: 6 }))
        if (index === i) syncSlider(id, n);
    });
  bindSlider('compression', (n) => {
    config.parameters.ratio = 1 + (n / 100) * 11;
    config.parameters.compressor_db = -12 - (n / 100) * 18;
    config.parameters.makeup_db = (n / 100) * 4;
    syncSlider('ratio', config.parameters.ratio, ':1');
    syncSlider('comp-threshold', config.parameters.compressor_db, ' dB');
    syncSlider('makeup', config.parameters.makeup_db, ' dB');
  });
  $('raw').onclick = () => setMonitorMode(true);
  $('processed').onclick = () => setMonitorMode(false);
  $('processing').onclick = () => {
    config.parameters.enabled = !config.parameters.enabled;
    render();
    commit(false);
  };
  $('bypass').onclick = () => {
    config.parameters.bypass = !config.parameters.bypass;
    render();
    commit();
  };
  $<HTMLSelectElement>('preset').onchange = () => {
    const preset = [...builtins, ...config.presets].find(
      (p) => p.name === $<HTMLSelectElement>('preset').value,
    );
    if (preset) {
      const enabled = config.parameters.enabled;
      config.parameters = structuredClone(preset.parameters);
      config.parameters.enabled = enabled;
      config.preset = preset.name;
      render();
      commit(false);
    }
  };
  $('calibrate').onclick = startCalibration;
  $('cancel-calibration').onclick = cancelCalibration;
  $('apply-calibration').onclick = () => {
    if (calibration.suggestion !== undefined) {
      config.parameters.gate_db = calibration.suggestion;
      config.parameters.gate_on = true;
      render();
      commit();
      cancelCalibration();
    }
  };
  $('save-preset').onclick = () => {
    const name = $<HTMLInputElement>('preset-name').value.trim();
    if (!name || builtins.some((p) => p.name === name) || name === 'Custom') {
      message('Choose a unique preset name.');
      return;
    }
    config.presets = config.presets.filter((p) => p.name !== name);
    config.presets.push({ name, parameters: structuredClone(config.parameters) });
    config.preset = name;
    render();
    commit(false);
  };
  $('delete-preset').onclick = () => {
    config.presets = config.presets.filter((p) => p.name !== config.preset);
    config.preset = 'Custom';
    render();
    commit(false);
  };
  $('export-preset').onclick = async () => {
    try {
      const path = await invoke<string>('export_preset', {
        preset: { name: config.preset, parameters: config.parameters },
      });
      message('Exported to ' + path);
    } catch (e) {
      message(String(e));
    }
  };
  $('import-preset').onclick = async () => {
    try {
      const preset = JSON.parse($<HTMLTextAreaElement>('import-json').value) as Preset;
      if (
        typeof preset.name !== 'string' ||
        !preset.name.trim() ||
        builtins.some((p) => p.name === preset.name) ||
        preset.name === 'Custom' ||
        !preset.parameters
      )
        throw Error('Use a unique name and include all sound parameters.');
      const next = structuredClone(config);
      next.presets = next.presets.filter((p) => p.name !== preset.name);
      next.presets.push(preset);
      clearTimeout(timer);
      await saving;
      await invoke('set_config', { config: next });
      config = (await invoke<Initial>('initial')).config;
      revision++;
      dirty = false;
      render();
      message('Preset imported. Select it in Your sound.');
    } catch (e) {
      message('Import failed: ' + String(e));
    }
  };
}
function syncSlider(id: string, n: number, unit = '') {
  const el = $<HTMLInputElement>(id);
  if (el) {
    el.value = String(n);
    $(id + '-value').textContent = number(n) + unit;
  }
}
function bindSlider(id: string, update: (n: number) => void) {
  const el = $<HTMLInputElement>(id);
  const unit = $(id + '-value').textContent!.replace(/^[-\d.]+/, '');
  el.oninput = () => {
    const n = Number(el.value);
    $(id + '-value').textContent = number(n) + unit;
    update(n);
    commit();
  };
}
function setMonitorMode(raw: boolean) {
  config.monitor_raw = raw;
  $('raw').classList.toggle('selected', raw);
  $('processed').classList.toggle('selected', !raw);
  commit(false);
}
function updateThreshold() {
  $('threshold').style.left = levelPosition(config.parameters.gate_db) + '%';
  $('threshold').hidden = !config.parameters.gate_on;
  $('threshold').setAttribute('aria-valuenow', String(config.parameters.gate_db));
  $('threshold-label').textContent = `GATE ${config.parameters.gate_db} dB`;
}
function options(
  el: HTMLSelectElement,
  devices: Device[],
  selected: string | null,
  defaultText: string,
) {
  el.innerHTML = `<option value="">${defaultText}</option>`;
  for (const d of devices) {
    const o = new Option(d.name + (d.is_default ? ' · default' : ''), d.id);
    el.add(o);
  }
  if (selected && !devices.some((d) => d.id === selected)) {
    el.add(new Option('Disconnected · ' + selected.split('::')[0], selected));
  }
  el.value = selected || '';
}
function update(s: Snapshot) {
  current = s;
  const d = s.diagnostics;
  const l = s.levels;
  // Check both endpoint halves; finding a name alone does not certify the driver.
  const virtualOutput = s.devices.outputs.find(
    (device) =>
      device.name.toLowerCase().includes('cable input') &&
      device.name.toLowerCase().includes('vb-audio'),
  );
  const virtualInput = s.devices.inputs.find(
    (device) =>
      device.name.toLowerCase().includes('cable output') &&
      device.name.toLowerCase().includes('vb-audio'),
  );
  const virtualReady = Boolean(virtualOutput && virtualInput);
  const installButton = $<HTMLButtonElement>('install-driver');
  installButton.disabled = virtualReady || driverInstalling || !driverPackage?.install_available;
  installButton.textContent = driverInstalling
    ? 'Installing…'
    : virtualReady
      ? 'VB-CABLE installed'
      : 'Install VB-CABLE';
  $('driver-install-status').textContent = driverInstalling
    ? 'Follow the Windows administrator prompt. Keep Luna Mic open until setup finishes.'
    : virtualReady
      ? 'VB-CABLE detected. Select CABLE Output as your microphone in Discord / OBS.'
      : driverPackage?.message || 'Checking driver package…';
  $<HTMLButtonElement>('use-virtual-mic').disabled = !virtualReady;
  $('virtual-status').textContent = virtualReady ? 'VB-CABLE detected' : 'VB-CABLE required';
  $('virtual-help').textContent = virtualReady
    ? 'Use CABLE Input here, then CABLE Output in Discord / OBS. Keep your physical microphone selected above.'
    : 'Install VB-CABLE before using Luna Mic.';
  const dependencyDialog = $<HTMLDialogElement>('dependency-dialog');
  $<HTMLButtonElement>('dependency-install').disabled = installButton.disabled;
  $('dependency-install').textContent = installButton.textContent;
  $('dependency-message').textContent = $('driver-install-status').textContent;
  if (virtualReady) {
    if (dependencyDialog.open) dependencyDialog.close();
    if (config.route_output !== virtualOutput!.id) {
      config.route_output = virtualOutput!.id;
      commit(false);
    }
  } else if (!dependencyDialog.open) dependencyDialog.showModal();
  const signature =
    JSON.stringify(s.devices) +
    config.input +
    config.monitor_output +
    config.route_output +
    config.input_channel;
  if (signature !== deviceSignature) {
    deviceSignature = signature;
    options(
      $('input'),
      s.devices.inputs,
      config.input,
      s.devices.inputs.length > 1 ? 'Choose your microphone…' : 'System default microphone',
    );
    options(
      $('monitor-output'),
      s.devices.outputs,
      config.monitor_output,
      'System default headphones / speakers',
    );
    options(
      $('route-output'),
      s.devices.outputs,
      config.route_output,
      'Meter only · no routed audio',
    );
    const mic =
      s.devices.inputs.find((x) => x.id === config.input) ??
      (config.input === null ? s.devices.inputs.find((x) => x.is_default) : undefined);
    $('device-name').textContent = mic?.name || 'Choose a microphone';
    $('device-info').textContent = mic
      ? `${mic.brand} · ${mic.sample_rate / 1000} kHz · ${mic.channels} ch${mic.is_default ? ' · Windows default' : ''}`
      : 'Select or reconnect a microphone';
    $<HTMLButtonElement>('make-default').disabled = config.input === null;
    const artwork = $<HTMLImageElement>('device-artwork');
    const source = mic ? microphoneArtwork(mic.name, mic.brand) : microphone;
    artwork.onerror = () => {
      artwork.onerror = null;
      artwork.src = microphone;
      artwork.alt = 'Generic microphone illustration';
    };
    artwork.src = source;
    artwork.alt =
      source === microphone ? 'Generic microphone illustration' : `${mic?.name} microphone`;
    const channel = $<HTMLSelectElement>('channel');
    channel.innerHTML = '';
    for (let i = 0; i < (mic?.channels || 1); i++)
      channel.add(new Option('Channel ' + (i + 1), String(i)));
    channel.value = String(config.input_channel);
  }
  $('active').textContent = d.running
    ? '● ACTIVE'
    : '● ' + (s.devices.inputs.length ? 'CHOOSE / RECONNECT' : 'NO MICROPHONE');
  $('active').classList.toggle('inactive', !d.running);
  if (l.clipped) clipUntil = performance.now() + 1200;
  $('clip').hidden = performance.now() > clipUntil;
  const db = d.running ? l.gate_input_db : -90;
  smoothed = db > smoothed ? db : smoothed * 0.65 + db * 0.35;
  $('meter-fill').style.clipPath = `inset(0 ${100 - levelPosition(smoothed)}% 0 0)`;
  $('meter-fill').setAttribute(
    'aria-valuenow',
    String(Math.round(Math.max(-80, Math.min(0, smoothed)))),
  );
  $('level-db').textContent = number(db) + ' dB';
  $('gate-peak').style.left = levelPosition(l.gate_detector_db) + '%';
  const suppression = $<HTMLInputElement>('suppression');
  suppression.disabled = !d.suppression_available;
  $<HTMLInputElement>('suppression-strength').disabled = !d.suppression_available;
  const method = d.noise_methods.find((method) => method.id === config.noise_method);
  for (const option of Array.from($<HTMLSelectElement>('noise-method').options)) {
    const status = d.noise_methods.find((method) => method.id === option.value);
    option.disabled = !!status && !status.available;
    if (status) option.textContent = status.name + (status.available ? '' : ' · unavailable');
  }
  $('suppression-info').textContent =
    d.noise_method !== config.noise_method
      ? 'Loading selected method; audio briefly restarts when changing engines…'
      : d.suppression_available
        ? `${method?.description || 'Local noise removal'}. Estimated frame delay: ${d.suppression_delay_ms || '—'} ms while active. Reduce strength if your voice sounds unnatural.`
        : 'Selected removal method is unavailable. Use a 48 kHz input and a build with that engine. Other effects still work.';
  const monitorWarning = config.monitor_raw
    ? 'You are hearing Original: noise removal and the gate are skipped. Choose With effects to hear cleanup.'
    : config.parameters.bypass
      ? 'Effects are off: you are hearing your original mic, without noise removal or gating.'
      : config.parameters.suppression_on && !d.suppression_available
        ? 'Noise removal is unavailable for this microphone format; the gate and other effects can still run.'
        : '';
  $('monitor-warning').textContent = monitorWarning;
  $('monitor-warning').hidden = !monitorWarning;
  $('output-db').textContent = 'Output ' + number(l.output_db) + ' dB';
  $('gate-state').textContent = config.parameters.bypass
    ? 'BYPASS · GATE SKIPPED'
    : !config.parameters.gate_on
      ? 'GATE OFF'
      : l.gate_open
        ? 'GATE OPEN'
        : 'GATE CLOSED';
  $('gate-state').classList.toggle('open', l.gate_open);
  $('reduction').textContent =
    l.reduction_db > 0.1 ? '−' + number(l.reduction_db) + ' dB compression' : '';
  $('limiter-state').textContent = l.limiter_active ? 'Limiting peaks' : 'Peak protection';
  // Include algorithm latency separately from the render queue and Windows buffers.
  $('diagnostics').textContent =
    `${d.input_name || 'No input'}\nStream: ${d.running ? 'running' : 'stopped'} · ${d.message}\n${d.warning}\nFormat: ${d.sample_rate} Hz / ${d.channels} channels\nInput callback: ${d.buffer_frames} frames (${number(d.sample_rate ? (d.buffer_frames / d.sample_rate) * 1000 : 0)} ms)\nDSP + routing callback work: ${d.callback_us} µs\nLargest output queue: ${number(d.queue_ms)} ms\nMonitor / route rate: ${d.monitor_rate} / ${d.route_rate} Hz\nUnderrun events / dropped samples: ${d.underruns} / ${d.overruns}\nModules: ${Object.entries(
      config.parameters,
    )
      .filter(([k, v]) => k.endsWith('_on') && v)
      .map(([k]) => k.replace('_on', ''))
      .join(', ')}\n${config.parameters.bypass ? 'All modules bypassed' : ''}`;
  if (!d.running && d.message) $('active').title = d.message;
  else $('active').title = d.warning || d.message;
  $('diagnostics').textContent +=
    `\nNoise removal: ${d.suppression_available ? 'available (48 kHz)' : 'unavailable'}\nSuppression frame delay: ${d.suppression_delay_ms} ms`;
  $('diagnostics').textContent +=
    `\nSelected method: ${d.noise_method}\nRemoval worker missed/error frames: ${l.suppression_problems}`;
  if (!d.running || d.warning) $('stream-message').textContent = d.warning || d.message;
  else $('stream-message').textContent = '';
  advanceCalibration(s);
}

const calibration: {
  stage: 0 | 1 | 2 | 3;
  start: number;
  background: number[];
  voice: number[];
  lastFrame: number;
  suggestion?: number;
} = { stage: 0, start: 0, background: [], voice: [], lastFrame: -1 };
function startCalibration() {
  if (!current?.diagnostics.running) {
    message('Connect and select a microphone before calibrating.');
    return;
  }
  Object.assign(calibration, {
    stage: 1,
    start: performance.now(),
    background: [],
    voice: [],
    lastFrame: -1,
    suggestion: undefined,
  });
  $('calibration').hidden = false;
  $('apply-calibration').hidden = true;
  $('calibrate').textContent = 'Calibrating…';
}
function cancelCalibration() {
  calibration.stage = 0;
  $('calibration').hidden = true;
  $('calibrate').textContent = 'Auto calibrate';
}
function percentile(values: number[], fraction: number) {
  const sorted = [...values].sort((a, b) => a - b);
  return sorted[Math.floor((sorted.length - 1) * fraction)];
}
function advanceCalibration(s: Snapshot) {
  if (calibration.stage !== 1 && calibration.stage !== 2) return;
  if (!s.diagnostics.running) {
    cancelCalibration();
    message('Calibration cancelled: microphone stream stopped.');
    return;
  }
  const elapsed = (performance.now() - calibration.start) / 1000;
  if (s.levels.frames !== calibration.lastFrame) {
    calibration.lastFrame = s.levels.frames;
    (calibration.stage === 1 ? calibration.background : calibration.voice).push(
      s.levels.gate_input_db,
    );
  }
  $('calibration-text').textContent =
    (calibration.stage === 1 ? 'Stay quiet' : 'Speak normally') +
    ` for ${Math.max(0, Math.ceil(5 - elapsed))} seconds.`;
  if (elapsed < 5) return;
  if (calibration.stage === 1) {
    calibration.stage = 2;
    calibration.start = performance.now();
    return;
  }
  const background = percentile(calibration.background, 0.8),
    voice = percentile(calibration.voice, 0.7);
  calibration.stage = 3;
  if (
    calibration.background.length < 25 ||
    calibration.voice.length < 25 ||
    voice - background < 8
  ) {
    $('calibration-text').textContent =
      'Voice and background are too similar, or too few samples arrived. Move closer to your mic and try again.';
    return;
  }
  calibration.suggestion = Math.round(
    Math.max(-80, Math.min(-5, background + Math.min(10, (voice - background) / 2))),
  );
  $('calibration-text').textContent =
    `Background: ${number(background)} dB · Voice: ${number(voice)} dB · Suggested gate: ${calibration.suggestion} dB. Fine-tune while watching the meter.`;
  $('apply-calibration').hidden = false;
  $('calibrate').textContent = 'Recalibrate';
}
async function poll() {
  if (!document.hidden && focused) {
    try {
      update(await invoke<Snapshot>('snapshot'));
    } catch (e) {
      message('Audio status unavailable: ' + String(e));
    }
  }
  setTimeout(poll, document.hidden || !focused ? 500 : 50);
}
async function start() {
  if (!isTauri()) {
    app.innerHTML =
      '<header><b>Luna Mic</b></header><section><h1>Run the desktop app</h1><p>This browser page cannot access the Rust audio engine.</p><p>From the project folder, run <code>npm run app</code>.</p></section>';
    return;
  }
  try {
    const init = await invoke<Initial>('initial');
    config = init.config;
    builtins = init.builtins;
    render();
    void refreshDriverPackage();
    await listen<string>('settings-error', (event) => message(event.payload));
    if (init.warning) message(init.warning);
    void poll();
  } catch (e) {
    app.textContent = 'Could not initialize Luna Mic: ' + String(e);
  }
}
// Tray changes are reflected when the window returns to focus. Do not overwrite
// a slider edit or an in-flight settings save.
window.addEventListener('blur', () => {
  focused = false;
});
window.addEventListener('focus', async () => {
  focused = true;
  if (!config || dirty) return;
  try {
    const init = await invoke<Initial>('initial');
    if (!dirty) {
      config = init.config;
      render();
    }
  } catch {
    /* Polling will surface a backend failure. */
  }
});
void start();
