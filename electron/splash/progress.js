// The progress rail in the slot under the title, and the line under it.
//
// The rail is wordless: three stretches of film edge, one per step of
// the boot (backend ready, flags decided, first page rendered), each
// lit as the main process says its step has landed
// (`window.splashProgress.set(n)`, n the count of steps landed;
// lib/splash.cjs). The line stays empty until a step has been pending
// `st` ms (`?st=`), counted from the page's load or the last step to
// land, then says what the main process gave for that step (`?t1=`..
// `?t3=`, in the app's language), and goes when it lands, unless the
// next step's line is the same words.
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
  const watch = () => {
    clearTimeout(timer);
    if (!status) return;
    const pending = done;
    const text = pending < segments.length ? texts[pending] : null;
    // A line already up that would say the same for the step now
    // pending stays up, rather than fading out and back in.
    if (text && status.classList.contains('on') && status.textContent === text) return;
    status.classList.remove('on');
    if (!text) return;
    timer = setTimeout(() => {
      status.textContent = texts[pending];
      status.classList.add('on');
    }, stallMs);
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
