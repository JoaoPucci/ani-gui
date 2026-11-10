// The page's language and its name for assistive technology, in the
// app's language, as the main process opened the page with them (`?l=`,
// `?al=`, lib/splash.cjs). Without them (a browser preview) the page
// keeps its English defaults. Anything that is not a language tag is
// ignored.
(() => {
  const q = new URLSearchParams(location.search);
  const lang = q.get('l');
  const label = q.get('al');
  if (lang && /^[A-Za-z]{2,3}(-[A-Za-z0-9]{2,8})*$/.test(lang)) document.documentElement.lang = lang;
  const main = document.querySelector('main');
  if (main && label) main.setAttribute('aria-label', label);
})();
