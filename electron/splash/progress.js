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
//
// The stuck state (lib/splash-stuck.cjs): the main process calls
// `stuck()` once the first page is late. The line takes the stuck
// words (`?sl=`) for good, the pending stretch is marked late, and the
// actions under it are filled (`?sr=` Restart, `?sq=` Quit) and shown.
// `stuck()` resolves with the action used, "restart" or "quit", after
// which the actions go inert; asked again, it offers them again. A page
// already complete, or opened without the words, offers nothing (null).
// When the rail completes after the stuck state, the actions go inert
// at once, so a click in the handover's last beat restarts nothing.
// Nothing is focused for the person and no key is bound: Tab reaches
// the buttons, which Enter and Space work as buttons do.
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
  const words = { line: q.get('sl'), restart: q.get('sr'), quit: q.get('sq') };
  // The stuck state: entered once; `pick` answers a pending stuck().
  let stuckOn = false;
  let pick = null;
  // The actions come after this script in the page, so they are found
  // when the stuck state needs them.
  const actions = () => document.querySelector('.actions');
  const settle = () => {
    pick = null;
    const a = actions();
    if (a) a.inert = true;
  };
  const say = (text) => {
    status.textContent = text;
    status.classList.add('on');
  };
  const watch = () => {
    clearTimeout(timer);
    if (!status || stuckOn) return;
    const pending = done;
    const text = pending < segments.length ? texts[pending] : null;
    const up = status.classList.contains('on');
    // A reason that has passed (the network, once the flags landed)
    // gives way to the generic line at once, never to a gap.
    if (up && generic && status.textContent !== generic && status.textContent !== text) say(generic);
    if (!text || (up && status.textContent === text)) return;
    timer = setTimeout(() => say(text), stallMs);
  };
  // The column grows by the line and the actions, and the stage
  // re-centres it; so it does not jump, it is measured before and after
  // and glides up from where it stood (splash.css, `rising`). Under
  // reduced motion the CSS drops the glide and it just moves.
  const enter = (a) => {
    stuckOn = true;
    clearTimeout(timer);
    const title = document.querySelector('.title');
    const from = title ? title.getBoundingClientRect().top : 0;
    say(words.line);
    if (done < segments.length) segments[done].classList.add('late');
    for (const button of a.querySelectorAll('button[data-act]')) {
      button.textContent = button.dataset.act === 'restart' ? words.restart : words.quit;
      button.addEventListener('click', () => {
        if (pick) pick(button.dataset.act);
      });
    }
    a.hidden = false;
    document.body.classList.add('stuck');
    if (!title) return;
    const rise = from - title.getBoundingClientRect().top;
    if (rise) {
      title.style.setProperty('--rise', `${rise}px`);
      title.classList.add('rising');
      title.addEventListener('animationend', () => title.classList.remove('rising'), { once: true });
    }
  };
  window.splashProgress = {
    set(n) {
      const to = Math.min(segments.length, Math.max(0, Math.floor(Number(n) || 0)));
      if (to > done) {
        for (let i = done; i < to; i++) {
          segments[i].classList.remove('late');
          segments[i].classList.add('lit');
        }
        done = to;
        // The pulse moves to the stretch now pending.
        if (stuckOn && done < segments.length) segments[done].classList.add('late');
        if (stuckOn && done >= segments.length) settle();
        watch();
      }
      return done;
    },
    stuck() {
      const a = actions();
      if (!a || !status || done >= segments.length || !words.line || !words.restart || !words.quit) return null;
      if (!stuckOn) enter(a);
      a.inert = false;
      return new Promise((resolve) => {
        pick = (choice) => {
          settle();
          resolve(choice);
        };
      });
    },
  };
  watch();
})();
