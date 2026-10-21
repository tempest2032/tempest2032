import { touchControls } from './touch.js';

const player = document.querySelector('#player');
const canvas = document.querySelector('#game');
const overlay = document.querySelector('#overlay');
const status = document.querySelector('#status');
const play = document.querySelector('#play');
const fullscreen = document.querySelector('#fullscreen');
const touch = touchControls(player, canvas);
function controls() {
  return touch.enabled
    ? '\n\nSlide your thumb in the left area to steer. Lift to stop.\nHold Fire to shoot; slide onto Jump or Zapper to use them.\nTap the playfield to pause, then tap Resume, Restart, or Exit.\nSettings is in the top-left corner of Welcome.\nSlide up/down in menus and warps. OK / Back in menus.'
    : '\n\nMove: arrows / A D · Fire: Space / Z\nJump: X · Superzapper: C\nPause / configuration: P · Mute: M\nFPS: F3 · Fullscreen: F\nConfirm: Enter · Back: Esc\n\nControllers supported. Press a controller button to connect.';
}
const audioContexts = new Set();
let loaded = false;
let engaged = false;
let started = false;
let failed = false;
let pendingSuspension = false;
let typingInitials = false;

// CPAL owns the audio context. Retain it so a user gesture can resume it.
const AudioContext = window.AudioContext || window.webkitAudioContext;
if (AudioContext) {
  window.AudioContext = new Proxy(AudioContext, {
    construct(target, args) {
      const context = Reflect.construct(target, args);
      audioContexts.add(context);
      return context;
    },
  });
}

function fail(error) {
  failed = true;
  engaged = false;
  touch.setActive(false);
  overlay.hidden = false;
  status.textContent = `The game could not run.\n${error?.message || String(error)}\nTry a browser with WebGL2 and hardware acceleration enabled.`;
  play.disabled = true;
  console.error(error);
}

window.tempestBrowser = {
  canvasWidth: () => canvas.getBoundingClientRect().width,
  touchInput: context => touch.read(context),
  active: () => {
    // Observe hide/resume even when both happened between animation frames.
    if (pendingSuspension) {
      pendingSuspension = false;
      return false;
    }
    return engaged && !document.hidden && document.hasFocus();
  },
  ready: () => {
    loaded = true;
    status.textContent = `${touch.enabled ? 'Tap' : 'Click'} Play to enable sound and focus the game.${controls()}`;
    play.disabled = false;
    fullscreen.disabled = false;
    if (navigator.standalone || matchMedia('(display-mode: fullscreen), (display-mode: standalone), (display-mode: minimal-ui)').matches) {
      engage();
    }
  },
  typingInitials: active => {
    typingInitials = active;
  },
  // Read before the wasm module starts: it decides the first antialiasing step.
  capabilities: () => {
    let renderer = '';
    try {
      const probe = document.createElement('canvas');
      probe.width = probe.height = 1;
      const gl = probe.getContext('webgl2') || probe.getContext('webgl');
      const info = gl && gl.getExtension('WEBGL_debug_renderer_info');
      if (info) renderer = String(gl.getParameter(info.UNMASKED_RENDERER_WEBGL) || '');
    } catch (error) {
      console.warn('Could not read the WebGL renderer:', error);
    }
    return { coarse: matchMedia('(pointer: coarse)').matches, renderer };
  },
};

function resumeAudio() {
  for (const context of audioContexts) {
    if (context.state === 'running' || context.state === 'closed') continue;
    context.resume().catch(error => {
      console.warn('Could not resume audio:', error);
    });
  }
}

function engage() {
  if (!loaded || failed || document.hidden) return;
  resumeAudio();
  engaged = true;
  started = true;
  overlay.hidden = true;
  touch.setActive(true);
  canvas.focus();
}

function suspend() {
  if (!engaged || failed) return;
  engaged = false;
  pendingSuspension = true;
  touch.setActive(false);
  overlay.hidden = false;
  status.textContent = `Game suspended. ${touch.enabled ? 'Tap' : 'Click'} Resume to continue.${controls()}`;
  play.textContent = 'Resume';
  for (const context of audioContexts) context.suspend().catch(console.warn);
}

play.addEventListener('click', engage);
// Installed apps can start before audio is allowed. Unlock it on normal input.
for (const event of ['pointerdown', 'pointerup']) {
  player.addEventListener(event, () => {
    if (engaged) resumeAudio();
  }, true);
}
window.addEventListener('blur', suspend);
document.addEventListener('visibilitychange', () => {
  if (document.hidden) suspend();
});
player.addEventListener('focusout', event => {
  if (!player.contains(event.relatedTarget)) suspend();
});
canvas.addEventListener('pointerdown', () => {
  if (started) engage();
});
document.addEventListener('keydown', event => {
  if (engaged) resumeAudio();
  if (!overlay.hidden && loaded && ['Enter', 'KeyP'].includes(event.code)) {
    event.preventDefault();
    event.stopImmediatePropagation();
    engage();
  }
}, true);
canvas.addEventListener('keydown', event => {
  if (event.code === 'KeyF' && started && !typingInitials
      && !event.repeat && !event.ctrlKey && !event.altKey && !event.metaKey) {
    event.preventDefault();
    event.stopImmediatePropagation();
    toggleFullscreen();
    return;
  }
  if (['F3', 'Space', 'ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown'].includes(event.code)) {
    event.preventDefault();
  }
});
async function toggleFullscreen() {
  try {
    if (document.fullscreenElement) await document.exitFullscreen();
    else await player.requestFullscreen();
    if (started) engage();
  } catch (error) {
    suspend();
    status.textContent = `Fullscreen is unavailable: ${error.message}`;
    console.warn(error);
  }
}
fullscreen.addEventListener('click', toggleFullscreen);
document.addEventListener('fullscreenchange', () => {
  fullscreen.textContent = document.fullscreenElement ? 'Exit fullscreen' : 'Fullscreen';
});
window.addEventListener('error', event => fail(event.error || event.message));
window.addEventListener('unhandledrejection', event => fail(event.reason));

if ('serviceWorker' in navigator) {
  window.addEventListener('load', () => {
    navigator.serviceWorker.register('./sw.js', { updateViaCache: 'none' }).catch(error => {
      console.warn('Offline installation is unavailable:', error);
    });
  });
}

// Generated bindings cache the audio constructor, so import them after wrapping it.
import('./tempest2032.js').then(({ default: init }) => init()).catch(fail);
