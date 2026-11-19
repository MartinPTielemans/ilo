import WebKit

/// The scripts ilo runs in every page, in a world of their own, apart from the page's scripts:
/// the page can't see them, call what they call, or read what they send. All but `print`, which
/// is the page's own to call, and sends nothing but that it was called.
enum PageScripts {
  /// The page called `print()`, which WebKit gives an app no other word of.
  static let printMessage = "iloPrint"
  /// What the page says of each touch, for `SwipeRule`.
  static let touchMessage = "iloTouch"
  /// What the page's fields hold, for `Unsent`.
  static let typedMessage = "iloTyped"

  static func add(to configuration: WKWebViewConfiguration, handler: any WKScriptMessageHandler) {
    let content = configuration.userContentController
    content.addUserScript(WKUserScript(source: touch, injectionTime: .atDocumentStart, forMainFrameOnly: false, in: .defaultClient))
    content.addUserScript(WKUserScript(source: typed, injectionTime: .atDocumentStart, forMainFrameOnly: true, in: .defaultClient))
    content.add(handler, contentWorld: .defaultClient, name: touchMessage)
    content.add(handler, contentWorld: .defaultClient, name: typedMessage)
    content.addUserScript(WKUserScript(source: print, injectionTime: .atDocumentStart, forMainFrameOnly: false, in: .page))
    content.add(handler, contentWorld: .page, name: printMessage)
    LoginScript.add(to: content, handler: handler)
  }

  /// The page's `print()`, which does nothing in a web view left to itself.
  private static let print = """
    (() => {
      const handler = window.webkit && webkit.messageHandlers && webkit.messageHandlers.iloPrint;
      if (handler) window.print = () => { handler.postMessage(null); };
    })();
    """

  /// Says, as a finger comes down, whether it's on something that takes a sideways gesture: a
  /// part of the page that scrolls sideways, a field, a slider, a map or any canvas, a frame, or
  /// anything under a `touch-action` that keeps sideways panning from the browser, which is how
  /// a page that moves things by pointer events has to say so. Then says so again if the page's
  /// own script keeps the touch or its first moves. The listeners are passive: they hold up
  /// nothing.
  private static let touch = """
    (() => {
      const post = ok => { try { webkit.messageHandlers.iloTouch.postMessage(ok); } catch (error) {} };
      const kinds = new Set(['INPUT', 'TEXTAREA', 'SELECT', 'CANVAS', 'VIDEO', 'AUDIO', 'IFRAME', 'EMBED', 'OBJECT']);
      const roles = new Set(['slider', 'scrollbar', 'application']);
      const takes = element => {
        if (kinds.has(element.tagName) || element.isContentEditable || element.getAttribute('draggable') === 'true') return true;
        if (roles.has(element.getAttribute('role')) || element.getAttribute('aria-roledescription') === 'carousel') return true;
        // What sliding galleries call themselves, for those that say it no other way.
        if (/(^|[\\s_-])(carousel|swiper|slider|slick|splide|glide|flickity|embla|slideshow)/i.test(element.getAttribute('class') || '')) return true;
        const style = getComputedStyle(element);
        const action = style.touchAction || 'auto';
        if (action !== 'auto' && action !== 'manipulation' && !/pan-x|pan-left|pan-right/.test(action)) return true;
        if (element !== document.documentElement && element !== document.body
            && element.scrollWidth > element.clientWidth + 1 && /auto|scroll|overlay/.test(style.overflowX)) return true;
        return style.scrollSnapType.startsWith('x') || style.scrollSnapType.startsWith('both') || style.scrollSnapType.startsWith('inline');
      };
      let moves = 0;
      // After the page's own listeners have run, however they stopped the event on its way.
      const after = event => setTimeout(() => { if (event.defaultPrevented) post(false); }, 0);
      addEventListener('touchstart', event => {
        moves = 0;
        const selection = getSelection();
        if (window !== top || event.touches.length !== 1 || (selection && !selection.isCollapsed)) return post(false);
        for (const node of event.composedPath()) {
          if (node instanceof Element && takes(node)) return post(false);
        }
        if (document.scrollingElement && document.scrollingElement.scrollWidth > innerWidth + 1) return post(false);
        post(true);
        after(event);
      }, { capture: true, passive: true });
      addEventListener('touchmove', event => {
        if (moves++ < 12) after(event);
      }, { capture: true, passive: true });
    })();
    """

  /// Tells the app what the user has typed and not sent into the page's text areas and its plain
  /// one-line fields that have an id or a name, and puts it back when asked. Never a field that
  /// is or ever was a password's or is drawn as one, another field of a form that ever held a
  /// password, a field outside any form on a page that ever held one, one a saved login was
  /// filled into, a payment card's or a one-time code's, numbers alone, a field of eight characters
  /// or fewer, or one the page marks autocomplete="off". Editable areas are left to their
  /// editors, which keep their own drafts.
  private static let typed = """
    (() => {
      if (globalThis.iloTyped) return;
      const post = message => { try { webkit.messageHandlers.iloTyped.postMessage(message); } catch (error) {} };
      const types = new Set(['text', 'search', 'email', 'url']);
      const secretName = /pass|pwd|card|cc-?(num|exp|csc|name)|cvv|cvc|csc|otp|one-?time|\\bpin\\b|ssn|secur|secret|token|captcha|iban|mnemonic|seed|recover|private|api.?key/i;
      // The fields that are or were a password's, the forms that held one, and their ids and names.
      const secret = new WeakSet();
      const secretForms = new WeakSet();
      const secretNames = new Set();
      // A password field with no form was seen: it has no form to tell which fields are its own.
      let secretLoose = false;
      // The fields a saved login was filled into, which the login script shares in this world.
      const filled = globalThis.iloFilled || (globalThis.iloFilled = new WeakSet());
      // Each field's defaultValue as its typing began.
      const began = new WeakMap();
      const sent = new WeakMap();
      const dirty = new Set();
      let last = '[]';
      let timer = 0;

      const formOf = element => element.form || element.closest('form');
      const masked = element => getComputedStyle(element).getPropertyValue('-webkit-text-security') !== 'none';
      const mark = element => {
        secret.add(element);
        const form = formOf(element);
        if (form) secretForms.add(form); else secretLoose = true;
        if (element.id) secretNames.add('#' + element.id);
        const name = element.getAttribute('name');
        if (name) secretNames.add('=' + name);
      };
      // Password fields are noted while they are ones: a tap on a "show" eye makes one a plain
      // field, or puts another in its place. The first 200 fields are looked at, for a huge page.
      const note = () => {
        const inputs = document.getElementsByTagName('input');
        for (let index = 0; index < inputs.length && index < 200; index++) {
          const element = inputs[index];
          if (!secret.has(element) && (element.type === 'password' || (types.has(element.type) && masked(element)))) mark(element);
        }
      };
      // Whether `value` typed into `element` is kept.
      const kept = (element, value) => {
        if (element.getRootNode() !== document || filled.has(element)) return false;
        const form = formOf(element);
        const auto = ((element.getAttribute('autocomplete') || '') + ' ' + ((form && form.getAttribute('autocomplete')) || '')).toLowerCase();
        if (/(^|\\s)off(\\s|$)|cc-|one-time-code|password/.test(auto)) return false;
        const name = element.getAttribute('name') || '';
        if (secretName.test(name + ' ' + element.id)) return false;
        if (element.tagName === 'TEXTAREA') return true;
        if (element.tagName !== 'INPUT') return false;
        if (element.type === 'password' || (types.has(element.type) && masked(element))) mark(element);
        if (secret.has(element) || !types.has(element.type)) return false;
        if (form && (secretForms.has(form) || form.querySelector('input[type=password]'))) return false;
        // Outside any form, every such field may be the username beside a password the page has or had.
        if (!form && (secretLoose || document.querySelector('input[type=password]'))) return false;
        if ((element.id && secretNames.has('#' + element.id)) || (name && secretNames.has('=' + name))) return false;
        if (element.maxLength >= 0 && element.maxLength <= 8) return false;
        // Numbers alone: a card's, a code, a PIN, a date, a phone number.
        return !/^[\\p{Nd}\\s\\-\\/.+()]*$/u.test(value);
      };
      // What a field is in its page: its kind, its id and name, and where its form is sent.
      const keyOf = element => {
        const name = element.getAttribute('name') || '';
        if (!element.id && !name) return null;
        const form = formOf(element);
        let path = '';
        if (form) { try { path = new URL(form.getAttribute('action') || '', location.href).pathname; } catch (error) {} }
        return JSON.stringify([element.tagName, element.type, element.id, name, form ? path : null]);
      };
      // The one field that is `key`. Two that are the same are neither.
      const find = key => {
        let parts;
        try { parts = JSON.parse(key); } catch (error) { return null; }
        if (!Array.isArray(parts) || (!parts[2] && !parts[3])) return null;
        const named = parts[2] ? document.querySelectorAll('[id="' + CSS.escape(parts[2]) + '"]') : document.getElementsByName(parts[3]);
        const found = [...named].filter(element => (element.tagName === 'INPUT' || element.tagName === 'TEXTAREA') && keyOf(element) === key);
        return found.length === 1 ? found[0] : null;
      };

      const collect = () => {
        clearTimeout(timer);
        timer = 0;
        note();
        const fields = [];
        for (const element of dirty) {
          if (!element.isConnected) { dirty.delete(element); continue; }
          const value = element.value;
          if (!value || !value.trim() || value.length > 100000 || value === sent.get(element) || value === element.defaultValue) continue;
          if (!kept(element, value)) continue;
          const key = keyOf(element);
          if (!key || find(key) !== element) continue;
          fields.push({ key, value, was: began.get(element) });
        }
        const now = JSON.stringify(fields);
        if (now === last) return;
        last = now;
        post({ url: location.href, fields });
      };
      const soon = () => { if (!timer) timer = setTimeout(collect, 400); };

      addEventListener('pointerdown', note, { capture: true, passive: true });
      addEventListener('focusin', note, { capture: true, passive: true });
      addEventListener('input', event => {
        const element = event.composedPath()[0];
        if (!(element instanceof Element) || (element.tagName !== 'INPUT' && element.tagName !== 'TEXTAREA')) return;
        // Before the untrusted are passed over: a password ilo filled in is a script's input too.
        if (element.type === 'password') { mark(element); return; }
        if (!event.isTrusted) return;
        if (!began.has(element)) began.set(element, element.defaultValue);
        dirty.add(element);
        soon();
      }, true);
      // A form sent takes its text with it: what it held then is no longer unsent.
      addEventListener('submit', event => {
        for (const element of dirty) {
          if (event.target instanceof Element && event.target.contains(element)) sent.set(element, element.value);
        }
        collect();
      }, true);
      // A page that sends by script empties its field after a key or a tap.
      addEventListener('keydown', event => { if (dirty.size && event.key === 'Enter') soon(); }, true);
      addEventListener('click', () => { if (dirty.size) soon(); }, true);
      addEventListener('pagehide', collect, true);
      document.addEventListener('visibilitychange', collect, true);

      // Puts a field's text back, if the field is as it was when its typing began: one the page
      // or the user has filled since keeps what it has. Says whether the field was there.
      const put = field => {
        const element = find(field.key);
        if (!element) return false;
        if (dirty.has(element) || element.value !== field.was || !kept(element, field.value)) return true;
        const kind = element.tagName === 'TEXTAREA' ? HTMLTextAreaElement : HTMLInputElement;
        Object.getOwnPropertyDescriptor(kind.prototype, 'value').set.call(element, field.value);
        element.dispatchEvent(new Event('input', { bubbles: true }));
        began.set(element, field.was);
        dirty.add(element);
        return true;
      };
      const restore = fields => {
        const page = location.href;
        let left = fields;
        // A page that changed its address without loading is another page.
        const all = () => {
          if (location.href !== page) return true;
          note();
          left = left.filter(field => !put(field));
          return left.length === 0;
        };
        if (all()) return;
        // A page that draws its fields late gets them filled as they appear, for a while.
        const watch = new MutationObserver(() => { if (all()) watch.disconnect(); });
        watch.observe(document.documentElement, { childList: true, subtree: true });
        setTimeout(() => watch.disconnect(), 10000);
      };
      globalThis.iloTyped = { collect, restore };
    })();
    """
}
