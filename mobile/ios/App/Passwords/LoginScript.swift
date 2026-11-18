import WebKit

/// The script that finds a page's sign-in fields, in ilo's own world with the other page
/// scripts: the page can't call it, hear what it tells the app, or reach what fills its fields.
enum LoginScript {
  /// Whether the page's own frame shows a password field.
  static let message = "iloLogin"
  /// The login a frame's form was sent with, and what became of that form after.
  static let usedMessage = "iloLoginUsed"

  static func add(to content: WKUserContentController, handler: any WKScriptMessageHandler) {
    content.addUserScript(WKUserScript(source: source, injectionTime: .atDocumentStart, forMainFrameOnly: true, in: .defaultClient))
    // A sign-in form may be in a frame of another site's, as a shop's is on its payment page's.
    content.addUserScript(WKUserScript(source: used, injectionTime: .atDocumentStart, forMainFrameOnly: false, in: .defaultClient))
    content.add(handler, contentWorld: .defaultClient, name: message)
    content.add(handler, contentWorld: .defaultClient, name: usedMessage)
  }

  /// Which fields are a sign-in's, for both scripts: what is saved is what would be filled.
  private static let fields = """
      const shown = element => {
        if (!element.isConnected) return false;
        const box = element.getBoundingClientRect();
        // A field too small to see, or parked off the page's edge, is there for scripts, not people.
        if (box.width < 8 || box.height < 8 || box.right + scrollX <= 0 || box.bottom + scrollY <= 0) return false;
        return element.checkVisibility({ opacityProperty: true, visibilityProperty: true });
      };
      const seen = element => !element.disabled && !element.readOnly && shown(element);
      const auto = element => (element.getAttribute('autocomplete') || '').toLowerCase();
      // Whether a field says by itself that it takes a username.
      const says = element => /username|email/.test(auto(element)) || element.type === 'email'
        || /user|email|login|account/i.test(element.id + ' ' + (element.getAttribute('name') || ''));
      const usernameField = password => {
        // With no form to say which fields are the sign-in's, only one that says it takes a
        // username is: the field before the password may be the page's search box.
        const fields = [...(password.form ? password.form.elements : document.querySelectorAll('input'))]
          .filter(element => element.tagName === 'INPUT' && ['text', 'email', 'tel'].includes(element.type) && seen(element)
            && (password.form || says(element)));
        const named = fields.find(element => /(^|\\s)(username|email)(\\s|$)/.test(auto(element)));
        if (named) return named;
        // Else the last text field before the password.
        let before = null;
        for (const field of fields) {
          if (field.compareDocumentPosition(password) & Node.DOCUMENT_POSITION_FOLLOWING) before = field;
        }
        return before;
      };
    """

  /// Tells the app when a password field that can be seen and typed into comes or goes, looking
  /// again only after the page changed or a field took the focus. `fill` puts a login into that
  /// field and the username field before it, as typing would for the page's own scripts, and
  /// only into the top frame's own document, at the origin the app says it is filling. Outside
  /// a form the username goes only into a field that says it is one, else nowhere.
  private static let source = """
    (() => {
      if (globalThis.iloLogin || window !== top) return;
      const post = shows => { try { webkit.messageHandlers.iloLogin.postMessage(shows); } catch (error) {} };
    \(fields)
      // Not a field that asks for a new password, as a sign-up form's does.
      const passwordField = () => [...document.querySelectorAll('input[type="password"]')]
        .find(element => seen(element) && !/new-password/.test(auto(element)));

      let last = null;
      let timer = 0;
      const look = () => {
        timer = 0;
        const shows = !!passwordField();
        if (shows === last) return;
        last = shows;
        post(shows);
      };
      const soon = () => { if (!timer) timer = setTimeout(look, 300); };
      new MutationObserver(soon).observe(document, { childList: true, subtree: true, attributes: true, attributeFilter: ['type'] });
      document.addEventListener('DOMContentLoaded', look);
      addEventListener('load', look);
      // A form the page had hidden shows by the time one of its fields takes the focus.
      addEventListener('focusin', soon, true);
      // A page brought back whole from the back-forward cache says so again.
      addEventListener('pageshow', event => { if (event.persisted) { last = null; look(); } });

      // The fields filled, for the script that keeps unsent text, in this world too, to pass over.
      const filled = globalThis.iloFilled || (globalThis.iloFilled = new WeakSet());
      const set = (element, value) => {
        filled.add(element);
        // The field's own setter, then the events a page's script waits for after typing.
        Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set.call(element, value);
        element.dispatchEvent(new Event('input', { bubbles: true }));
        element.dispatchEvent(new Event('change', { bubbles: true }));
      };
      const fill = (origin, username, password) => {
        if (window !== top || location.origin !== origin) return 'moved';
        const field = passwordField();
        if (!field) return 'gone';
        const user = usernameField(field);
        if (user && username) set(user, username);
        set(field, password);
        return 'filled';
      };
      globalThis.iloLogin = { fill };
    })();
    """

  /// Reads the username and password a form is sent with, in whichever frame it is, at the
  /// moment it's sent: a form's submit, or for a page that signs in without one, its button or
  /// Return in its field. Then says once what became of the field: that it left the page, as
  /// after a sign-in that took, or that it's typed in again or drawn again, as after one that
  /// didn't. Nothing is read from a form with no password in it, or with a card's or a one-time
  /// code's field. It watches the page only after a form was sent.
  private static let used = """
    (() => {
      if (globalThis.iloLoginUsed || !/^https?:$/.test(location.protocol)) return;
      globalThis.iloLoginUsed = true;
      const post = body => { try { webkit.messageHandlers.iloLoginUsed.postMessage(body); } catch (error) {} };
    \(fields)
      // A field that took a password is still one once the page shows what was typed in it.
      const was = new WeakSet();
      const isPassword = element => element.type === 'password' || was.has(element);
      const called = element => element.id + ' ' + (element.getAttribute('name') || '');
      // A card's number or code, or a code good for one sign-in: nothing to keep.
      const once = element => /cc-|one-time-code/.test(auto(element))
        || /card.?(number|num|no)([^a-z]|$)|cvc|cvv|(^|[^a-z])(csc|otp|totp)([^a-z]|$)|one.?time|verification.?code/i.test(called(element));
      // A username the page keeps where nobody sees it, as the second page of a sign-in does.
      const kept = element => element.value && !isPassword(element) && (/(^|\\s)(username|email)(\\s|$)/.test(auto(element))
        || element.type === 'email' || (element.type === 'hidden' && /^(user(name)?|login|loginfmt|email|identifier)$/i.test(element.getAttribute('name') || '')));
      const read = scope => {
        // A form's own fields, or those of the page that are in no form.
        const all = [...(scope === document ? document.querySelectorAll('input') : scope.elements)]
          .filter(element => element.tagName === 'INPUT' && (scope !== document || !element.form));
        if (all.some(element => element.type !== 'hidden' && once(element))) return null;
        const passwords = all.filter(element => isPassword(element) && seen(element) && element.value);
        if (!passwords.length) return null;
        // Of several the new one: the one that says so, else the last, after the one it replaces.
        const field = passwords.find(element => /new-password/.test(auto(element))) || passwords[passwords.length - 1];
        const user = usernameField(field) || all.find(kept);
        const login = { username: user ? user.value : '', password: field.value };
        return login.username.length > 1000 || login.password.length > 1000 ? null : { field, login };
      };

      let held = null;
      let sent = '';
      let observer = null;
      let timer = 0;
      const stop = () => {
        if (observer) observer.disconnect();
        observer = null;
        held = null;
      };
      const check = () => {
        timer = 0;
        if (!held || shown(held)) return;
        // The form drawn again in its place is a sign-in that didn't take.
        const back = [...document.querySelectorAll('input[type="password"]')].some(shown);
        stop();
        post(back ? { edited: true } : { left: true });
      };
      const use = scope => {
        const found = read(scope);
        if (!found) return;
        // A button's tap and its form's submit are one sending.
        const now = found.login.username + '\\n' + found.login.password;
        if (held === found.field && sent === now) return;
        held = found.field;
        sent = now;
        post(found.login);
        if (observer) return;
        observer = new MutationObserver(() => { if (!timer) timer = setTimeout(check, 300); });
        observer.observe(document, { childList: true, subtree: true, attributes: true, attributeFilter: ['class', 'style', 'hidden', 'type'] });
      };
      addEventListener('input', event => {
        const element = event.target;
        if (!element || element.tagName !== 'INPUT') return;
        if (element.type === 'password') was.add(element);
        // Typed in again after it was sent: the sign-in didn't take.
        if (element === held && event.isTrusted) {
          stop();
          post({ edited: true });
        }
      }, true);
      addEventListener('submit', event => { if (event.target && event.target.tagName === 'FORM') use(event.target); }, true);
      addEventListener('keydown', event => {
        const element = event.target;
        if (event.key === 'Enter' && element && element.tagName === 'INPUT') use(element.form || document);
      }, true);
      addEventListener('click', event => {
        const element = event.target instanceof Element ? event.target : null;
        const button = element && element.closest('button, input[type="submit"], input[type="button"], [role="button"]');
        if (button) use(button.form || button.closest('form') || document);
      }, true);
    })();
    """
}
