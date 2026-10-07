// Small HTML helpers shared by the layout and live update code.
// Device and preset names must be escaped; labels are trusted app-owned markup.
export const esc = (s: string) =>
  s.replace(
    /[&<>"']/g,
    (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c]!,
  );
export const number = (n: number) => (Number.isFinite(n) ? n.toFixed(1) : '—');
export const levelPosition = (n: number) => Math.max(0, Math.min(100, ((n + 80) / 80) * 100));
export function toggle(id: string, label: string, checked: boolean, disabled = false) {
  return `<label class="toggle"><span>${label}</span><input id="${id}" type="checkbox" ${checked ? 'checked' : ''} ${disabled ? 'disabled' : ''}><span class="switch" aria-hidden="true"></span></label>`;
}
export function slider(
  id: string,
  label: string,
  value: number,
  min: number,
  max: number,
  step = 1,
  unit = '',
) {
  return `<label class="slider-row" for="${id}"><span>${label}</span><input id="${id}" type="range" min="${min}" max="${max}" step="${step}" value="${value}"><output id="${id}-value">${number(value)}${unit}</output></label>`;
}
