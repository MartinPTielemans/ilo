// Runs a page's procedural filters, as uBlock Origin's content script does: `filters`, adblock-rust's
// {selector, action} each, matched with procedural.js's selectors. One with no action hides what
// it finds, and one with :style() styles it, by an attribute that the page's user style sheet
// names: `hidden`, and in `styled` one for each style. The others take out the element, an
// attribute or a class. Run again on the frame after the page adds or takes out elements, never
// on a timer, so an idle page runs nothing.
const run = (filters, hidden, styled) => {
  const compiled = [];
  for (const { selector, action } of filters) {
    try {
      compiled.push({ selector: compileProceduralSelector(selector), action });
    } catch {
      // An operator procedural.js doesn't have: this filter alone does nothing, as in uBlock Origin.
    }
  }
  const mark = (element, name) => {
    if (name && !element.hasAttribute(name)) {
      element.setAttribute(name, '');
    }
  };
  const act = (element, action) => {
    if (!action) {
      mark(element, hidden);
    } else if (action.type === 'style') {
      mark(element, styled[action.arg]);
    } else if (action.type === 'remove') {
      element.remove();
    } else if (action.type === 'remove-attr') {
      element.removeAttribute(action.arg);
    } else if (action.type === 'remove-class' && element.classList.contains(action.arg)) {
      element.classList.remove(action.arg);
    }
  };
  const apply = () => {
    for (const { selector, action } of compiled) {
      let found;
      try {
        found = applyCompiledSelector(selector);
      } catch {
        continue;
      }
      for (const element of found) {
        act(element, action);
      }
    }
  };
  let pending = false;
  const later = () => {
    if (!pending) {
      pending = true;
      requestAnimationFrame(() => {
        pending = false;
        apply();
      });
    }
  };
  if (compiled.length) {
    apply();
    // The attributes it sets aren't watched, so marking an element doesn't run it again.
    new MutationObserver(later).observe(document, { childList: true, subtree: true });
  }
};
