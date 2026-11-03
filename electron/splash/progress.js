// The progress rail in the slot under the title, and the line under it.
//
// The rail is wordless: three stretches of film edge, one per step of
// the boot (backend ready, flags decided, first page rendered), each
// lit as the main process says its step has landed
// (`window.splashProgress.set(n)`, n the count of steps landed;
// lib/splash.cjs). The line stays empty until a step has been pending
// `st` ms (`?st=`), counted from the page's load or the last step to
// land, then says what the main process gave for that step (`?t1=`..
// `?t3=`, in the app's language). Once up it stays until the splash
// closes; only its words change, when the reason does. A step whose
// own line is not the generic one (`?tg=`; the network, for the flags
// step) shows its words only once it has stalled, and when it lands the
// generic line takes over at once.
// A page opened without them (a browser preview) never shows the line.
// How each change moves, or does not under reduced motion, is
// splash.css's.
(() => {
  const q = new URLSearchParams(location.search);
  const segments = document.querySelectorAll('.rail i');
  const status = document.querySelector('.status');
  const texts = [q.get('t1'), q.get('t2'), q.get('t3')];
  const stallMs = Number(q.get('st')) || 700;
  let done = 0;
  let timer = null;
  const generic = q.get('tg');
  const say = (text) => {
    status.textContent = text;
    status.classList.add('on');
  };
  const watch = () => {
    clearTimeout(timer);
    if (!status) return;
    const pending = done;
    const text = pending < segments.length ? texts[pending] : null;
    const up = status.classList.contains('on');
    // A reason that has passed (the network, once the flags landed)
    // gives way to the generic line at once, never to a gap.
    if (up && generic && status.textContent !== generic && status.textContent !== text) say(generic);
    if (!text || (up && status.textContent === text)) return;
    timer = setTimeout(() => say(text), stallMs);
  };
  window.splashProgress = {
    set(n) {
      const to = Math.min(segments.length, Math.max(0, Math.floor(Number(n) || 0)));
      if (to > done) {
        for (let i = done; i < to; i++) segments[i].classList.add('lit');
        done = to;
        watch();
      }
      return done;
    },
  };
  watch();
})();
